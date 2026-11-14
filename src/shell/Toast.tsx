import { useEffect, useState } from "react";
import { useUi } from "../store/ui";

const SHOW_MS = 4000;

/** The latest message from `notify`, bottom centre, for four seconds. */
export function Toast() {
  const toast = useUi((s) => s.toast);
  const [visible, setVisible] = useState<number | null>(null);

  useEffect(() => {
    if (!toast) return;
    setVisible(toast.id);
    const timer = setTimeout(() => setVisible(null), SHOW_MS);
    return () => clearTimeout(timer);
  }, [toast]);

  if (!toast || visible !== toast.id) return null;
  return (
    <div className="toast" role="status">
      {toast.text}
    </div>
  );
}
