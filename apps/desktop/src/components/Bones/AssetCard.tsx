import { useEffect, useState } from "react";
import { ImageOff, Wrench, Car, Home, Box } from "lucide-react";
import * as ipc from "../../lib/asset/ipc";
import type { Asset, AssetCategory } from "../../lib/asset/ipc";

const CATEGORY_ICONS: Record<AssetCategory, typeof Wrench> = {
  appliance: Wrench,
  vehicle: Car,
  fixture: Home,
  other: Box,
};

interface Props {
  asset: Asset;
  onClick: () => void;
}

export function AssetCard({ asset, onClick }: Props) {
  const [heroSrc, setHeroSrc] = useState<string | null>(null);

  useEffect(() => {
    const uuid = asset.hero_attachment_uuid;
    setHeroSrc(null);
    if (uuid) { void ipc.attachmentSrc(uuid).then(setHeroSrc).catch(() => {}); }
  }, [asset.hero_attachment_uuid]);

  const Icon = CATEGORY_ICONS[asset.category];
  const year = asset.purchase_date ? new Date(asset.purchase_date + "T00:00:00").getFullYear() : null;

  return (
    <button
      onClick={onClick}
      style={{
        textAlign: "left",
        background: "var(--paper, #fff)",
        border: "1px solid var(--hairline, #e5e5e5)",
        borderRadius: 6,
        padding: 0,
        cursor: "pointer",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div style={{
        aspectRatio: "4 / 3",
        background: "var(--paper-muted, #f5f5f5)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        {heroSrc ? (
          <img src={heroSrc} alt={asset.name}
            style={{ width: "100%", height: "100%", objectFit: "cover" }} />
        ) : (
          <ImageOff size={32} strokeWidth={1.4} color="var(--ink-soft, #999)" />
        )}
      </div>
      <div style={{ padding: 12 }}>
        <div style={{
          fontSize: 16, fontWeight: 600,
          whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
        }}>
          {asset.name}
        </div>
        {asset.make && (
          <div style={{ fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 2,
                        whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {asset.make}
          </div>
        )}
        <div style={{ display: "flex", alignItems: "center", gap: 4,
                      fontSize: 12, color: "var(--ink-soft, #999)", marginTop: 4 }}>
          <Icon size={12} strokeWidth={1.8} />
          {year != null && <span>{year}</span>}
        </div>
      </div>
    </button>
  );
}
