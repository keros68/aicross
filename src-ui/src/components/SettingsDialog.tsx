import type { Settings, TabSettings } from "../types";

const tabLabels: Array<[keyof TabSettings, string]> = [
  ["workspace", "工作区"],
  ["resources", "资源"],
  ["sessions", "记录"],
  ["changes", "修改记录"],
  ["plan", "方案与编排"],
  ["tasks", "任务列表"],
];

export function SettingsDialog({
  value,
  onChange,
  onSave,
  onClose,
}: {
  value: Settings;
  onChange: (settings: Settings) => void;
  onSave: () => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal" role="dialog" aria-modal="true" aria-labelledby="settings-title" onMouseDown={(event) => event.stopPropagation()}>
        <h2 id="settings-title">设置</h2>
        <label className="field-label">
          默认 shell
          <select value={value.defaultShell} onChange={(event) => onChange({ ...value, defaultShell: event.target.value })}>
            <option value="pwsh.exe">PowerShell 7</option>
            <option value="powershell.exe">Windows PowerShell</option>
            <option value="cmd.exe">命令提示符</option>
            <option value="sh">sh</option>
          </select>
        </label>
        <fieldset>
          <legend>显示的 tab</legend>
          {tabLabels.map(([key, label]) => (
            <label className="check-row" key={key}>
              <input
                type="checkbox"
                checked={value.tabs[key]}
                onChange={(event) => onChange({ ...value, tabs: { ...value.tabs, [key]: event.target.checked } })}
              />
              {label}
            </label>
          ))}
        </fieldset>
        <div className="modal-actions">
          <button className="button" onClick={onClose}>取消</button>
          <button className="button primary" onClick={onSave}>保存</button>
        </div>
      </section>
    </div>
  );
}

