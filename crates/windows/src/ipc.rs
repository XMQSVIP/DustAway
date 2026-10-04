//! One-shot authenticated local pipe, UI is server, elevated helper is client.
//! The elevated client explicitly uses SECURITY_IDENTIFICATION (not impersonation).
use crate::{
    fs::*,
    platform::{self, WindowsPlatform},
    security::{self, Descriptor},
};
use anyhow::{Context, Result, ensure};
use dustaway_core::*;
use serde::{Deserialize, Serialize};
use std::{
    mem::size_of,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
use windows::{
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
        UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
    },
    core::{PCWSTR, w},
};

const MAX_FRAME: usize = 16 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Envelope {
    nonce: String,
    request: Request,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "command",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    Hello,
    WindowsScan,
    WindowsSnapshot,
    WindowsPrepare {
        session: String,
        ids: Vec<String>,
    },
    WindowsValidate {
        plan_id: String,
    },
    WindowsExecute {
        plan_id: String,
    },
    WindowsCancel,
    WindowsHistory,
    Scan {
        rule_ids: Vec<String>,
    },
    Status,
    Summary,
    Observations {
        session: String,
        rule: String,
        offset: u64,
        limit: u32,
    },
    Snapshot,
    Candidates {
        session: String,
        rule: String,
        offset: u64,
        limit: u32,
        #[serde(default)]
        query: CandidateQuery,
    },
    Refresh {
        session: String,
    },
    Prepare {
        session: String,
        selection: Selection,
    },
    Validate {
        plan_id: String,
    },
    Execute {
        plan_id: String,
    },
    Report,
    History,
    Cancel,
    Shutdown,
    Settings {
        settings: Settings,
    },
}
#[derive(Serialize, Deserialize)]
struct Response {
    value: Option<serde_json::Value>,
    error: Option<String>,
}

struct Pipe(Handle);
impl Pipe {
    fn send<T: Serialize>(&self, value: &T) -> Result<()> {
        let payload = serde_json::to_vec(value)?;
        ensure!(payload.len() <= MAX_FRAME, "通信消息过大");
        let mut packet = (payload.len() as u32).to_le_bytes().to_vec();
        packet.extend(payload);
        let mut offset = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while offset < packet.len() {
            ensure!(Instant::now() < deadline, "辅助进程写入超时");
            let mut written = 0;
            unsafe {
                WriteFile(self.0.0, Some(&packet[offset..]), Some(&mut written), None)?;
            }
            offset += written as usize;
            if written == 0 {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        Ok(())
    }
    fn read_exact(&self, data: &mut [u8], timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut offset = 0;
        while offset < data.len() {
            let mut available = 0;
            unsafe {
                PeekNamedPipe(self.0.0, None, 0, None, Some(&mut available), None)?;
            }
            if available == 0 {
                ensure!(Instant::now() < deadline, "pipe-timeout");
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }
            let length = (data.len() - offset).min(available as usize);
            let mut read = 0;
            unsafe {
                ReadFile(
                    self.0.0,
                    Some(&mut data[offset..offset + length]),
                    Some(&mut read),
                    None,
                )?;
            }
            ensure!(read > 0, "辅助进程连接已关闭");
            offset += read as usize;
        }
        Ok(())
    }
    fn receive<T: serde::de::DeserializeOwned>(&self, timeout: Duration) -> Result<T> {
        let mut size = [0u8; 4];
        self.read_exact(&mut size, timeout)?;
        let size = u32::from_le_bytes(size) as usize;
        ensure!(size > 0 && size <= MAX_FRAME, "通信消息长度无效");
        let mut payload = vec![0u8; size];
        self.read_exact(&mut payload, Duration::from_secs(10))?;
        Ok(serde_json::from_slice(&payload)?)
    }
}

pub struct HelperClient {
    pipe: Pipe,
    nonce: String,
    _process: Handle,
}
impl HelperClient {
    pub fn launch(helper: &Path, expected_sha256: &str) -> Result<Self> {
        ensure!(!security::elevated()?, "主程序必须以普通用户权限运行");
        ensure!(helper.is_file(), "未找到系统清理组件，请使用完整安装包");
        let pin = PinnedPath::open(helper, false, false)?;
        verify_helper_digest(&pin, expected_sha256)?;
        let id = Uuid::new_v4().to_string();
        let nonce = Uuid::new_v4().to_string();
        let name = text_wide(&format!("\\\\.\\pipe\\DustAway-{id}"));
        let sid = security::current_sid()?;
        let acl = Descriptor::from_sddl(&format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{sid})"))?;
        let attrs = acl.attributes();
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65536,
                65536,
                0,
                Some(&attrs),
            )
        };
        ensure!(handle != INVALID_HANDLE_VALUE, "无法创建受限通信通道");
        let pipe = Pipe(Handle(handle));
        let executable = wide(helper);
        let args = text_wide(&format!(
            "--pipe {id} --nonce {nonce} --parent {}",
            std::process::id()
        ));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(executable.as_ptr()),
            lpParameters: PCWSTR(args.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        unsafe { ShellExecuteExW(&mut info) }
            .context("系统清理授权被取消或组件无法启动；仍可清理用户文件")?;
        let process = Handle(info.hProcess);
        let pid = unsafe { GetProcessId(process.0) };
        ensure!(pid != 0, "无法验证辅助进程");
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match unsafe { ConnectNamedPipe(pipe.0.0, None) } {
                Ok(()) => break,
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => break,
                Err(e) if e.code() == ERROR_PIPE_LISTENING.to_hresult() => {}
                Err(e) => return Err(e.into()),
            }
            ensure!(Instant::now() < deadline, "等待系统清理组件连接超时");
            ensure!(
                unsafe { WaitForSingleObject(process.0, 0) } == WAIT_TIMEOUT,
                "系统清理组件已退出"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        let mut peer = 0;
        unsafe {
            GetNamedPipeClientProcessId(pipe.0.0, &mut peer)?;
        }
        ensure!(
            peer == pid && security::elevated_process(process.0)?,
            "辅助进程身份验证失败"
        );
        ensure!(
            security::process_session(peer)? == security::process_session(std::process::id())?,
            "会话身份不匹配"
        );
        ensure!(
            extended(&security::process_image(process.0)?)? == extended(helper)?,
            "辅助进程路径不匹配"
        );
        drop(pin);
        let client = Self {
            pipe,
            nonce,
            _process: process,
        };
        let hello: serde_json::Value = client.call(Request::Hello)?;
        ensure!(
            hello["protocol"] == 2,
            "系统组件版本不匹配，请重新安装完整安装包"
        );
        Ok(client)
    }
    pub fn call<T: serde::de::DeserializeOwned>(&self, request: Request) -> Result<T> {
        self.pipe.send(&Envelope {
            nonce: self.nonce.clone(),
            request,
        })?;
        let response: Response = self.pipe.receive(Duration::from_secs(30))?;
        if let Some(error) = response.error {
            anyhow::bail!(error);
        }
        Ok(serde_json::from_value(
            response.value.context("辅助进程响应无效")?,
        )?)
    }
}
fn verify_helper_digest(pin: &PinnedPath, expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|c| c.is_ascii_hexdigit()),
        "安装版缺少辅助组件校验信息"
    );
    let mut file = std::fs::File::open(&pin.path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    ensure!(
        format!("{:x}", digest.finalize()).eq_ignore_ascii_case(expected),
        "辅助组件已变化，请重新安装配套安装包；普通用户清理仍可使用"
    );
    Ok(())
}
impl Drop for HelperClient {
    fn drop(&mut self) {
        let _ = self.pipe.send(&Envelope {
            nonce: self.nonce.clone(),
            request: Request::Shutdown,
        });
    }
}

pub fn serve_helper() -> Result<()> {
    ensure!(security::elevated()?, "辅助进程需要管理员权限");
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 6 && args[0] == "--pipe" && args[2] == "--nonce" && args[4] == "--parent",
        "无效启动参数"
    );
    Uuid::parse_str(&args[1])?;
    Uuid::parse_str(&args[3])?;
    let parent_id: u32 = args[5].parse()?;
    let parent = Handle(unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            parent_id,
        )?
    });
    ensure!(
        !security::elevated_process(parent.0)?,
        "不接受来自提权界面的请求"
    );
    ensure!(
        security::process_session(parent_id)? == security::process_session(std::process::id())?,
        "会话身份不匹配"
    );
    let own = std::env::current_exe()?;
    let expected = own.parent().context("无效安装路径")?.join("dustaway.exe");
    ensure!(
        extended(&security::process_image(parent.0)?)? == extended(&expected)?,
        "仅允许同一安装目录中的 DustAway 连接"
    );
    if !cfg!(debug_assertions) {
        let program_files = platform::known_folder(&FOLDERID_ProgramFiles)?;
        ensure!(
            extended(&own)?.starts_with(extended(&program_files)?),
            "系统清理需要安装到 Program Files"
        );
    }
    let _parent_pin = PinnedPath::open(&expected, false, false)?;
    let name = text_wide(&format!("\\\\.\\pipe\\DustAway-{}", args[1]));
    // SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION prevents the medium-integrity
    // pipe server from impersonating the elevated helper's token.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0x0011_0000),
            None,
        )?
    };
    let pipe = Pipe(Handle(handle));
    let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
    unsafe {
        SetNamedPipeHandleState(pipe.0.0, Some(&mode), None, None)?;
    }
    let mut server = 0;
    unsafe {
        GetNamedPipeServerProcessId(pipe.0.0, &mut server)?;
    }
    ensure!(server == parent_id, "管道服务器身份不匹配");
    let journal_dir = create_system_journal_dir()?;
    let sid = security::user_sid(parent.0)?;
    let journal_path = journal_dir.path.join(format!("system-{sid}.sqlite"));
    let maintenance =
        crate::system_maintenance::WindowsMaintenance::new(Scope::System, &journal_dir.path)?;
    for suffix in ["", "-wal", "-shm"] {
        let file = journal_dir
            .path
            .join(format!("system-{sid}.sqlite{suffix}"));
        if file.exists() {
            let _verified = PinnedPath::open(&file, false, false)?;
        }
    }
    let engine = Engine::open(
        Arc::new(WindowsPlatform::new(Scope::System)?),
        Scope::System,
        &journal_path,
    )?;
    let mut last_activity = Instant::now();
    let result = (|| -> Result<()> {
        loop {
            ensure!(
                unsafe { WaitForSingleObject(parent.0, 0) } == WAIT_TIMEOUT,
                "主程序已退出"
            );
            let envelope: Envelope = match pipe.receive(Duration::from_secs(1)) {
                Ok(v) => v,
                Err(e) if e.to_string() == "pipe-timeout" => {
                    if !engine.is_busy()
                        && !maintenance.is_busy()
                        && last_activity.elapsed() > Duration::from_secs(300)
                    {
                        break;
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };
            ensure!(envelope.nonce == args[3], "会话凭据无效");
            last_activity = Instant::now();
            if matches!(envelope.request, Request::Shutdown) {
                break;
            }
            let response = match dispatch_windows(&engine, &maintenance, envelope.request) {
                Ok(value) => Response {
                    value: Some(value),
                    error: None,
                },
                Err(error) => Response {
                    value: None,
                    error: Some(format!("{error:#}")),
                },
            };
            pipe.send(&response)?;
        }
        Ok(())
    })();
    engine.cancel();
    maintenance.cancel();
    while engine.is_busy() || maintenance.is_busy() {
        std::thread::sleep(Duration::from_millis(25));
    }
    result
}

fn dispatch_windows(
    engine: &Arc<Engine>,
    maintenance: &Arc<crate::system_maintenance::WindowsMaintenance>,
    request: Request,
) -> Result<serde_json::Value> {
    Ok(match request {
        Request::WindowsSnapshot => serde_json::to_value(maintenance.snapshot())?,
        Request::WindowsHistory => serde_json::to_value(maintenance.history()?)?,
        Request::WindowsScan => {
            ensure!(!engine.is_busy(), "逐文件任务进行中");
            serde_json::to_value(maintenance.scan(
                crate::system_maintenance::settings_exclusions(&engine.settings()),
            )?)?
        }
        Request::WindowsPrepare { session, ids } => {
            ensure!(!engine.is_busy(), "逐文件任务进行中");
            maintenance.prepare(
                session,
                ids,
                crate::system_maintenance::settings_exclusions(&engine.settings()),
            )?;
            serde_json::Value::Null
        }
        Request::WindowsValidate { plan_id } => {
            maintenance.validate_plan(
                &plan_id,
                &crate::system_maintenance::settings_exclusions(&engine.settings()),
            )?;
            serde_json::Value::Null
        }
        Request::WindowsExecute { plan_id } => {
            ensure!(!engine.is_busy(), "逐文件任务进行中");
            serde_json::to_value(maintenance.execute(
                plan_id,
                crate::system_maintenance::settings_exclusions(&engine.settings()),
            )?)?
        }
        Request::WindowsCancel => {
            maintenance.cancel();
            serde_json::Value::Null
        }
        Request::Cancel => {
            maintenance.cancel();
            dispatch(engine, Request::Cancel)?
        }
        request => {
            ensure!(
                !maintenance.is_busy()
                    || matches!(
                        &request,
                        Request::Hello
                            | Request::Status
                            | Request::Summary
                            | Request::Snapshot
                            | Request::Report
                            | Request::History
                            | Request::Candidates { .. }
                            | Request::Observations { .. }
                    ),
                "Windows 维护任务进行中"
            );
            dispatch(engine, request)?
        }
    })
}
fn dispatch(engine: &Arc<Engine>, request: Request) -> Result<serde_json::Value> {
    Ok(match request {
        Request::Hello => serde_json::json!({"protocol":2,"scope":"system"}),
        Request::WindowsScan
        | Request::WindowsSnapshot
        | Request::WindowsPrepare { .. }
        | Request::WindowsValidate { .. }
        | Request::WindowsExecute { .. }
        | Request::WindowsCancel
        | Request::WindowsHistory => anyhow::bail!("Windows 维护组件未初始化"),
        Request::Scan { rule_ids } => serde_json::to_value(engine.start_scan(rule_ids)?)?,
        Request::Status => serde_json::to_value(engine.status())?,
        Request::Summary => serde_json::to_value(engine.summary())?,
        Request::Observations {
            session,
            rule,
            offset,
            limit,
        } => serde_json::to_value(engine.observations(&session, &rule, offset, limit)?)?,
        Request::Snapshot => serde_json::to_value(engine.runtime_snapshot())?,
        Request::Candidates {
            session,
            rule,
            offset,
            limit,
            query,
        } => serde_json::to_value(engine.query_candidates(&session, &rule, offset, limit, query)?)?,
        Request::Refresh { session } => {
            serde_json::to_value(engine.refresh_availability(&session)?)?
        }
        Request::Prepare { session, selection } => {
            serde_json::to_value(engine.prepare(&session, selection)?)?
        }
        Request::Validate { plan_id } => {
            engine.validate_plan(&plan_id)?;
            serde_json::Value::Null
        }
        Request::Execute { plan_id } => serde_json::to_value(engine.execute(&plan_id)?)?,
        Request::Report => serde_json::to_value(engine.report())?,
        Request::History => serde_json::to_value(engine.history()?)?,
        Request::Cancel | Request::Shutdown => {
            engine.cancel();
            serde_json::Value::Null
        }
        Request::Settings { settings } => {
            for path in &settings.exclusions {
                extended(Path::new(path))?;
            }
            engine.save_settings(settings)?;
            serde_json::Value::Null
        }
    })
}

fn create_system_journal_dir() -> Result<PinnedPath> {
    let parent = platform::known_folder(&FOLDERID_ProgramData)?;
    let _pin = PinnedPath::open(&parent, true, false)?;
    let path = parent.join("DustAway-System");
    let expected = "O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";
    let acl = Descriptor::from_sddl(expected)?;
    let attrs = acl.attributes();
    let name = wide(&path);
    if !path.exists() {
        unsafe {
            CreateDirectoryW(PCWSTR(name.as_ptr()), Some(&attrs))?;
        }
    }
    let pin = PinnedPath::open(&path, true, false)?;
    // Never "repair" a pre-planted writable directory: it may contain planted DB links.
    ensure!(
        security::owner_and_dacl(&path)? == expected,
        "系统日志目录权限不可信，已拒绝系统清理"
    );
    Ok(pin)
}

#[cfg(test)]
mod tests {
    #[test]
    fn helper_replacement_is_rejected_before_elevation() {
        use super::*;
        use sha2::{Digest, Sha256};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("helper.exe");
        std::fs::write(&path, b"original bundled component").unwrap();
        let expected = format!("{:x}", Sha256::digest(b"original bundled component"));
        {
            let pin = PinnedPath::open(&path, false, false).unwrap();
            verify_helper_digest(&pin, &expected).unwrap();
            assert!(verify_helper_digest(&pin, "").is_err());
        }
        std::fs::write(&path, b"replacement component").unwrap();
        let pin = PinnedPath::open(&path, false, false).unwrap();
        assert!(verify_helper_digest(&pin, &expected).is_err());
    }
    use super::*;
    #[test]
    fn protocol_has_no_arbitrary_path_or_shell_command() {
        for command in [
            "delete",
            "shell",
            "powershell",
            "load_rules",
            "import_database",
        ] {
            let payload = serde_json::json!({"command": command, "args": {"path": "C:\\private\\document.txt"}});
            assert!(serde_json::from_value::<Request>(payload).is_err());
        }
    }
    #[test]
    fn helper_engine_rejects_user_rules_and_external_plan_ids() {
        let temp = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            Arc::new(WindowsPlatform::new(Scope::System).unwrap()),
            Scope::System,
            &temp.path().join("system-fixture.sqlite"),
        )
        .unwrap();
        assert!(
            dispatch(
                &engine,
                Request::Scan {
                    rule_ids: vec!["windows.user_temp".into()]
                }
            )
            .is_err()
        );
        assert!(
            dispatch(
                &engine,
                Request::Execute {
                    plan_id: Uuid::new_v4().to_string()
                }
            )
            .is_err()
        );
        assert_eq!(engine.status().state, JobState::Idle);
        assert!(engine.history().unwrap().is_empty());
    }
}
