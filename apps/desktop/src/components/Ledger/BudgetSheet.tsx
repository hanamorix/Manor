import type { Budget, Category } from "../../lib/ledger/ipc";

interface Props {
  categories: Category[];
  budgets: Budget[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}

export default function BudgetSheet(_props: Props) {
  return null;
}
