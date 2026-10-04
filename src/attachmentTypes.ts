export type AttachmentKind = 'image' | 'video' | 'audio' | 'file' | 'unknown';
export type AttachmentProvider = 'weixin' | 'qq_classic' | 'qqnt';
export const attachmentProviderLabels: Record<AttachmentProvider, string> = {
  weixin: '微信',
  qq_classic: '经典 QQ',
  qqnt: 'QQNT',
};
export function attachmentSourceLabel(source: string | undefined): string | undefined {
  return (
    {
      'weixin.attachments': '微信',
      'qq.attachments': '经典 QQ',
      'qqnt.attachments': 'QQNT',
    } as Record<string, string>
  )[source ?? ''];
}
export interface AttachmentDiscovery {
  provider: AttachmentProvider;
  locations: AttachmentLocation[];
  state: string;
  issues: string[];
}
export interface AttachmentLocation {
  provider: AttachmentProvider;
  token: string;
  path: string;
  title: string;
  version: string;
  cleanup_block: string | null;
}
export interface AttachmentEntry {
  id: string;
  name: string;
  path: string;
  kind: AttachmentKind;
  bytes: string;
  modified_unix: number;
  preview_available: boolean;
  cleanup_block: string | null;
  outcome: string | null;
}
export interface AttachmentSnapshot {
  session: string;
  location: AttachmentLocation;
  state: string;
  files: number;
  bytes: string;
  issues: number;
  error: string | null;
  deleted_ids: string[];
}
export interface AttachmentQuery {
  session: string;
  kind: AttachmentKind | null;
  min_age_days: number;
  search: string;
  offset: number;
  selected_ids: string[] | null;
}
export interface AttachmentPage {
  entries: AttachmentEntry[];
  total: number;
  filtered_bytes?: string;
  selectable_files?: number;
  selectable_bytes?: string;
  categories?: {
    kind: AttachmentKind;
    files: number;
    bytes: string;
    retained_files: number;
    retained_bytes: string;
  }[];
  snapshot: AttachmentSnapshot;
}
export interface AttachmentSelection {
  session: string;
  selected_ids: string[];
}
export const attachmentKindLabels: Record<AttachmentKind, string> = {
  image: '图片',
  video: '视频',
  audio: '语音',
  file: '接收文件',
  unknown: '未知格式',
};
