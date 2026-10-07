// HostBar.tsx — skill 与模式两页工具行左边的主 agent 厂商（TASK-51；fix-51-r3 精简为
// 一行）：「厂商 Anthropic · 官方端点」+ 小按钮「修改」，点开是厂商下拉（自动推定 /
// 各已知厂商）。依据（vendorBasis）直接作为「·」后的短文字；「未知」用异常色。

import { useState } from 'react';
import { commands, type HostEntry } from '../../tauri';
import { vendorOptions, vendorSelectValue, vendorText } from '../../lib/host-select';

export function HostBar({ host, onVendorChanged }: { host: HostEntry; onVendorChanged: () => void }) {
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const vendor = vendorText(host);
  const basis = host.vendorBasis.trim();

  const change = async (value: string) => {
    setBusy(true);
    setError('');
    try {
      await commands.hostSetVendor(host.id, value === '' ? 'auto' : value);
      setEditing(false);
      onVendorChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="host-bar">
      <span className="host-bar-line" title={basis === '' ? undefined : basis}>
        厂商 <span className={vendor.alert ? 'cfg-alert-text host-bar-value' : 'host-bar-value'}>{vendor.text}</span>
        {basis !== '' && <span className="host-bar-basis"> · {basis}</span>}
      </span>
      {editing ? (
        <select
          className="host-bar-select"
          aria-label="厂商"
          autoFocus
          value={vendorSelectValue(host)}
          disabled={busy}
          onChange={(event) => void change(event.target.value)}
          onBlur={() => { if (!busy) setEditing(false); }}
        >
          {vendorOptions(host).map((option) => (
            <option key={option.value} value={option.value}>{option.label}</option>
          ))}
        </select>
      ) : (
        <button type="button" className="btn small" onClick={() => setEditing(true)}>修改</button>
      )}
      {error !== '' && <span className="cfg-error host-bar-error">{error}</span>}
    </div>
  );
}
