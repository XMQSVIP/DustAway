import type { Bootstrap } from './types';
import capabilities from '../docs/current-capabilities.json';

/** Build catalogue describes permission boundaries, never this machine's available bytes. */
export function ProductCapabilities({ boot }: { boot: Bootstrap }) {
  const matchingBuild = boot.version.replace(/\+.*$/, '') === capabilities.version;
  return (
    <div className="product-capabilities">
      <p>当前为未签名的小范围测试版。扫描不会删除文件；清理须经核验和一次永久删除确认。</p>
      <dl>
        <dt>常规清理</dt>
        <dd>
          {matchingBuild
            ? `${capabilities.user_rules} 条用户规则：${capabilities.file_cleanup_rules} 条按条件逐文件清理、${capabilities.official_tool_rules} 条官方工具操作、${capabilities.readonly_rules} 条只读。`
            : '版本信息不一致，请以本次扫描与后端核验结果为准。'}
          规则数量不代表本机已安装软件或可释放容量。
        </dd>
        <dt>聊天附件</dt>
        <dd>微信、经典 QQ 与 QQNT 分别核验来源、账号、布局和文件格式；在独立清单中选择和确认。</dd>
        <dt>软件卸载与残留</dt>
        <dd>使用官方卸载入口；卸载后的残留另行核验，未开放或受保护的内容保留。</dd>
        <dt>系统维护</dt>
        <dd>
          {boot.capabilities.system_cleanup
            ? '本构建支持受限系统维护，需要时单独申请管理员授权。'
            : '本构建不提供提权清理，也不包含系统辅助组件。'}
        </dd>
      </dl>
    </div>
  );
}
