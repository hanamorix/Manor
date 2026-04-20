import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useMaintenanceEventsStore } from "../../lib/maintenance/event-state";
import type { LedgerTransaction } from "../../lib/maintenance/event-ipc";

interface Props {
  completedDate: string;
  costPence: number | null;
  selectedTransactionId: number | null;
  excludeEventId: string | null;
  onSelect(txId: number | null, tx: LedgerTransaction | null): void;
}

function formatAmount(pence: number): string {
  const abs = Math.abs(pence);
  return `£${(abs / 100).toFixed(2)}`;
}

function formatDate(unixSec: number): string {
  return new Date(unixSec * 1000).toLocaleDateString("en-GB", {
    month: "short",
    day: "numeric",
  });
}

export function TransactionSuggest({
  completedDate,
  costPence,
  selectedTransactionId,
  excludeEventId,
  onSelect,
}: Props) {
  const { suggestTransactions, searchTransactions } = useMaintenanceEventsStore();
  const [candidates, setCandidates] = useState<LedgerTransaction[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<LedgerTransaction[] | null>(null);
  const [selectedTx, setSelectedTx] = useState<LedgerTransaction | null>(null);
  const debounceRef = useRef<number | null>(null);

  useEffect(() => {
    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
    }
    debounceRef.current = window.setTimeout(() => {
      suggestTransactions(completedDate, costPence, excludeEventId)
        .then(setCandidates)
        .catch((e) => {
          console.error("TransactionSuggest: suggest failed", e);
        });
    }, 300);
    return () => {
      if (debounceRef.current !== null) {
        window.clearTimeout(debounceRef.current);
      }
    };
  }, [completedDate, costPence, excludeEventId, suggestTransactions]);

  useEffect(() => {
    if (selectedTransactionId && !selectedTx) {
      const hit = candidates.find((t) => t.id === selectedTransactionId);
      if (hit) setSelectedTx(hit);
    }
    if (!selectedTransactionId) setSelectedTx(null);
  }, [selectedTransactionId, candidates, selectedTx]);

  const runSearch = (q: string) => {
    setSearchQuery(q);
    if (q.trim().length < 2) {
      setSearchResults(null);
      return;
    }
    searchTransactions(q).then(setSearchResults).catch(console.error);
  };

  const rowsToShow = searchResults ?? candidates;

  if (selectedTx) {
    return (
      <div style={{
        padding: 8,
        background: "var(--surface-elevated, #f6f6f6)",
        borderRadius: 6,
        display: "flex",
        alignItems: "center",
        gap: 8,
      }}>
        <span style={{ flex: 1 }}>
          ✓ {selectedTx.description} · {formatAmount(selectedTx.amount_pence)} · {formatDate(selectedTx.date)}
        </span>
        <button
          type="button"
          onClick={() => {
            onSelect(null, null);
            setSelectedTx(null);
          }}
          title="Unlink"
          aria-label="Unlink transaction"
          style={{ background: "none", border: "none", cursor: "pointer" }}
        >
          <X size={14} />
        </button>
      </div>
    );
  }

  return (
    <div>
      {rowsToShow.length === 0 ? (
        <div style={{ color: "var(--ink-soft, #888)", fontSize: 13 }}>
          No matching transactions.
        </div>
      ) : (
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {rowsToShow.map((tx) => (
            <li key={tx.id}>
              <button
                type="button"
                onClick={() => {
                  onSelect(tx.id, tx);
                  setSelectedTx(tx);
                }}
                style={{
                  display: "block",
                  width: "100%",
                  padding: 8,
                  textAlign: "left",
                  background: "none",
                  border: "1px solid var(--border, #e5e5e5)",
                  borderRadius: 6,
                  marginBottom: 4,
                  cursor: "pointer",
                }}
              >
                {tx.merchant ?? tx.description} · {formatAmount(tx.amount_pence)} · {formatDate(tx.date)}
              </button>
            </li>
          ))}
        </ul>
      )}
      <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 6 }}>
        <Search size={14} strokeWidth={1.6} />
        <input
          type="text"
          placeholder="None of these — search all…"
          value={searchQuery}
          onChange={(e) => runSearch(e.target.value)}
          style={{
            flex: 1,
            padding: 6,
            border: "1px solid var(--border, #e5e5e5)",
            borderRadius: 4,
          }}
        />
      </div>
    </div>
  );
}
