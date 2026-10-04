use anyhow::{Result, ensure};
use dustaway_core::*;
use serde::Serialize;

/// Held under Service.control for the entire command, including its receipt.
/// A reconciliation advances the version before a late command can enter.
pub struct ControlState {
    runtime_id: String,
    revision: u64,
    receipt: Option<CommandReceipt>,
    pub closing: bool,
}
impl Default for ControlState {
    fn default() -> Self {
        Self {
            runtime_id: uuid::Uuid::new_v4().to_string(),
            revision: 0,
            receipt: None,
            closing: false,
        }
    }
}
impl ControlState {
    pub fn info(&self) -> ControlInfo {
        ControlInfo {
            runtime_id: self.runtime_id.clone(),
            revision: self.revision.to_string(),
            last_request: self.receipt.clone(),
        }
    }
    fn check(&self, request: &CommandRequest) -> Result<()> {
        ensure!(
            request.runtime_id == self.runtime_id,
            ApiError::new("runtime_changed", "程序运行实例已变化，请重新加载")
        );
        ensure!(
            uuid::Uuid::parse_str(&request.request_id).is_ok(),
            ApiError::new("invalid_request", "请求标识无效")
        );
        Ok(())
    }
    fn advance(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| ApiError::new("request_expired", "控制版本已耗尽，请重新启动"))?;
        Ok(())
    }
    pub fn run<T: Serialize>(
        &mut self,
        request: &CommandRequest,
        command: &str,
        work: impl FnOnce() -> Result<T>,
    ) -> Result<ControlInfo> {
        self.check(request)?;
        ensure!(!self.closing, ApiError::new("closing", "程序正在退出"));
        if let Some(receipt) = &self.receipt
            && receipt.request_id == request.request_id
        {
            ensure!(
                receipt.command == command,
                ApiError::new("request_expired", "请求标识已使用")
            );
            return Ok(self.info());
        }
        ensure!(
            request.expected_revision == self.revision.to_string(),
            ApiError::new("request_expired", "请求已失效，请核对当前状态")
        );
        self.advance()?;
        // Even an unexpected panic consumes the version; it must never be replayed.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
            .unwrap_or_else(|_| {
                Err(ApiError::new("worker_failed", "命令异常中断，请核对任务状态").into())
            })
            .and_then(|value| Ok(serde_json::to_value(value)?));
        let (value, error) = match outcome {
            Ok(value) => (value, None),
            Err(error) => (
                serde_json::Value::Null,
                Some(serde_json::to_value(classify_api_error(error))?),
            ),
        };
        self.receipt = Some(CommandReceipt {
            request_id: request.request_id.clone(),
            command: command.into(),
            value,
            error,
        });
        Ok(self.info())
    }
    pub fn reconcile(&mut self, request: Option<&CommandRequest>) -> Result<()> {
        if let Some(request) = request {
            self.check(request)?;
            if self
                .receipt
                .as_ref()
                .is_some_and(|r| r.request_id == request.request_id)
            {
                return Ok(());
            }
            if request.expected_revision != self.revision.to_string() {
                return Ok(());
            }
            self.advance()?;
            self.receipt = Some(CommandReceipt {
                request_id: request.request_id.clone(),
                command: "sealed".into(),
                value: serde_json::Value::Null,
                error: Some(serde_json::to_value(ApiError::new(
                    "request_expired",
                    "请求未受理，已封存；可重新操作",
                ))?),
            });
        } else {
            // Page attachment fences commands still queued by the old page.
            self.advance()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};
    fn request(c: &ControlState) -> CommandRequest {
        CommandRequest {
            runtime_id: c.runtime_id.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: c.revision.to_string(),
        }
    }
    #[test]
    fn accepted_receipt_is_recovered_without_repeating_work() {
        let mut c = ControlState::default();
        let r = request(&c);
        c.run(&r, "execute", || Ok("accepted")).unwrap();
        c.reconcile(Some(&r)).unwrap();
        let reply = c
            .run(&r, "execute", || -> Result<()> { panic!("replayed") })
            .unwrap();
        assert_eq!(reply.last_request.unwrap().value, "accepted");
    }
    #[test]
    fn reconciliation_fences_a_command_waiting_to_enter() {
        let c = Arc::new(Mutex::new(ControlState::default()));
        let r = request(&c.lock().unwrap());
        let gate = Arc::new(Barrier::new(2));
        let worker = {
            let c = c.clone();
            let r = r.clone();
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                c.lock()
                    .unwrap()
                    .run(&r, "scan", || -> Result<()> { panic!("late command ran") })
            })
        };
        c.lock().unwrap().reconcile(Some(&r)).unwrap();
        gate.wait();
        assert!(worker.join().unwrap().is_err());
    }
    #[test]
    fn reload_and_new_runtime_reject_old_requests() {
        let mut c = ControlState::default();
        let r = request(&c);
        c.reconcile(None).unwrap();
        assert!(c.run(&r, "save", || Ok(())).is_err());
        assert!(ControlState::default().run(&r, "save", || Ok(())).is_err());
    }
    #[test]
    fn failure_and_panic_consume_the_version() {
        let mut c = ControlState::default();
        let r = request(&c);
        let reply = c
            .run(&r, "save", || -> Result<()> {
                anyhow::bail!("write failed")
            })
            .unwrap();
        assert!(reply.last_request.unwrap().error.is_some());
        let newer = request(&c);
        c.run(&newer, "prepare", || -> Result<()> {
            panic!("worker panicked")
        })
        .unwrap();
        assert!(c.run(&r, "save", || Ok(())).is_err());
    }
}
