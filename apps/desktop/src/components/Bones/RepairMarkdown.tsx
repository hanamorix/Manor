import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { open as openUrl } from "@tauri-apps/plugin-shell";

interface Props {
  body: string;
}

export function RepairMarkdown({ body }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ href, children, ...rest }) => (
          <a
            {...rest}
            href={href}
            onClick={(e) => {
              e.preventDefault();
              if (href) {
                void openUrl(href);
              }
            }}
            style={{ color: "var(--link, #0366d6)", textDecoration: "underline", cursor: "pointer" }}
          >
            {children}
          </a>
        ),
      }}
    >
      {body}
    </ReactMarkdown>
  );
}
