import type { Category, Transaction } from "../../lib/ledger/ipc";

interface Props {
  transactions: Transaction[];
  categories: Category[];
  onAdd: () => void;
}

export default function TransactionFeed(_props: Props) {
  return <div>Transaction feed — coming in next task</div>;
}
