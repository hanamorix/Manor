import type { Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export default function AddTransactionForm(_props: Props) {
  return null;
}
