interface Props {
  schedule?: unknown;
  initialAssetId?: string;
  lockAsset?: boolean;
  onClose: () => void;
  onSaved: () => void;
  onDeleted?: () => void;
}
export function ScheduleDrawer({ onClose }: Props) {
  return <div onClick={onClose}>Stub — Task 9</div>;
}
