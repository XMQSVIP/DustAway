import { candidateActions } from './i18n';
import * as Dialog from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import type { ExclusionDraft } from './types';
import { useDialogFocus } from './dialogFocus';
export function exclusionPath(draft: ExclusionDraft) {
  return draft.kind === 'file' ? draft.file.path : draft.file.path.replace(/[\\/][^\\/]+$/, '');
}
export function ExclusionsDialog({
  drafts,
  saving,
  onClose,
  onSave,
  onRemove,
  error,
}: {
  error: string;
  drafts: ExclusionDraft[];
  saving: boolean;
  onClose: () => void;
  onSave: () => void;
  onRemove: (index: number) => void;
}) {
  const dialogFocus = useDialogFocus();
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open && !saving) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="overlay" />
        <Dialog.Content className="dialog exclusions-dialog unfolded-sheet" {...dialogFocus}>
          <div className="dialog-heading">
            <div>
              <Dialog.Title>{candidateActions.review}</Dialog.Title>
              <Dialog.Description>{candidateActions.description}</Dialog.Description>
            </div>
            <Dialog.Close className="icon-button" aria-label="关闭排除草稿" disabled={saving}>
              <X size={20} />
            </Dialog.Close>
          </div>
          <div className="draft-list" tabIndex={0} aria-label="排除路径列表">
            {drafts.map((d, index) => (
              <div className="draft-item" key={`${d.session}:${d.file.id}:${d.kind}`}>
                <div>
                  <strong>{d.kind === 'file' ? '文件' : '文件夹及其全部内容'}</strong>
                  <p className="full-path">{exclusionPath(d)}</p>
                  {d.kind === 'parent' &&
                    d.file.root &&
                    exclusionPath(d).toLowerCase().replaceAll('/', '\\') ===
                      d.file.root.toLowerCase().replaceAll('/', '\\') && (
                      <small className="error-text">
                        这是该配置的整个缓存根目录；其中所有文件都将保留。
                      </small>
                    )}
                </div>
                <button
                  className="icon-button"
                  disabled={saving}
                  onClick={() => onRemove(index)}
                  aria-label={`移除草稿 ${exclusionPath(d)}`}
                >
                  <X size={18} />
                </button>
              </div>
            ))}
            {!drafts.length && <p>没有待保存的排除项。</p>}
          </div>
          {error && (
            <p role="alert" className="error-text">
              {error}
            </p>
          )}
          <div className="dialog-actions">
            <Dialog.Close className="button secondary" disabled={saving}>
              返回文件明细
            </Dialog.Close>
            <button
              className="button primary"
              disabled={saving || !drafts.length || drafts.length > 500}
              onClick={onSave}
            >
              {saving ? candidateActions.saving : candidateActions.save}
            </button>
          </div>
          {drafts.length > 500 && <p role="alert">最多添加 500 个排除项，请移除部分草稿。</p>}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
