import { Save, Trash2 } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { useUi } from "../../store/ui";
import { Hint } from "../../ui/controls";

/** Profiles: load one (format, its settings and the effects), save the current ones under a name, delete. */
export function ProfileRow() {
  const { t } = useTranslation();
  const id = useId();
  const profiles = useSettings((s) => s.settings?.profiles ?? []);
  const update = useSettings((s) => s.update);
  const options = useEncode((s) => s.options);
  const [name, setName] = useState("");
  const trimmed = name.trim();
  const exists = profiles.some((p) => p.name === trimmed);

  const load = (chosen: string) => {
    const p = profiles.find((x) => x.name === chosen);
    if (!p) return;
    setName(p.name);
    update((s) => {
      s.codec = p.codec;
      s.enc = structuredClone(p.enc);
      s.effects = structuredClone(p.effects);
    });
  };

  const save = () => {
    if (!trimmed) return;
    update((s) => {
      s.profiles = s.profiles.filter((x) => x.name !== trimmed);
      s.profiles.push({
        name: trimmed,
        codec: s.codec,
        enc: structuredClone(s.enc),
        effects: structuredClone(s.effects ?? []),
      });
    });
    useUi.getState().notify(t("Profile “{name}” saved", { name: trimmed }));
  };

  const remove = () => {
    update((s) => {
      s.profiles = s.profiles.filter((x) => x.name !== trimmed);
    });
    setName("");
    useUi.getState().notify(t("Profile “{name}” deleted", { name: trimmed }));
  };

  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {t("Profile")}
      </label>
      <select
        id={id}
        className="dd"
        value={exists ? trimmed : ""}
        disabled={profiles.length === 0}
        onChange={(e) => load(e.target.value)}
      >
        <option value="" disabled>
          {profiles.length === 0 ? t("No profiles yet") : t("Load profile …")}
        </option>
        {profiles.map((p) => (
          <option key={p.name} value={p.name}>
            {`${p.name} (${codecInfo(options, p.codec)?.short ?? p.codec})`}
          </option>
        ))}
      </select>
      <div className="row" style={{ marginTop: 8 }}>
        <input
          className="text-field"
          type="text"
          aria-label={t("Profile name")}
          placeholder={t("Profile name")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") save();
          }}
        />
        <button
          type="button"
          className="icon-btn"
          aria-label={t("Save profile")}
          title={t("Save profile")}
          disabled={!trimmed}
          onClick={save}
        >
          <Save size={16} />
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label={t("Delete this profile")}
          title={t("Delete this profile")}
          disabled={!exists}
          onClick={remove}
        >
          <Trash2 size={16} />
        </button>
      </div>
      <Hint>{t("A profile keeps the format, its settings and the effects.")}</Hint>
    </div>
  );
}
