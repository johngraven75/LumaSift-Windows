import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ALL_SELECTION_TYPES,
  Candidate,
  CompanionAccess,
  NasConnectRequest,
  ResolutionPlan,
  ScanProgress,
  SelectionType,
  selectionLabel
} from "./types";

const idleProgress: ScanProgress = {
  scanning: false,
  phase: "Ready",
  current: 0,
  total: 0,
  percentage: 0,
  filesConsidered: 0,
  message: "Choose sources and file types. LumaSift will build a review-only plan before moving anything."
};

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let amount = value / 1024;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount.toFixed(amount >= 10 ? 0 : 1)} ${units[unit]}`;
}

function candidateClass(candidate: Candidate): string {
  return candidate.disposition === "retain" ? "candidate retain" : "candidate queued";
}

export function LumaSiftApp(): JSX.Element {
  const [sources, setSources] = useState<string[]>([]);
  const [selectedTypes, setSelectedTypes] = useState<SelectionType[]>(ALL_SELECTION_TYPES);
  const [progress, setProgress] = useState<ScanProgress>(idleProgress);
  const [plan, setPlan] = useState<ResolutionPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [purgeText, setPurgeText] = useState("");
  const [companion, setCompanion] = useState<CompanionAccess | null>(null);
  const [nas, setNas] = useState<NasConnectRequest>({
    uncPath: "",
    username: "",
    password: "",
    rememberConnection: false
  });

  const progressText = useMemo(
    () => `${Math.max(0, Math.min(100, progress.percentage))}% · ${progress.phase}`,
    [progress.percentage, progress.phase]
  );

  useEffect(() => {
    void invoke<CompanionAccess>("get_companion_access").then(setCompanion).catch((reason) => setError(String(reason)));
  }, []);

  useEffect(() => {
    if (!progress.scanning) return;
    const timer = window.setInterval(() => {
      void refreshProgress();
    }, 600);
    return () => window.clearInterval(timer);
  }, [progress.scanning]);

  async function refreshProgress(): Promise<void> {
    try {
      const next = await invoke<ScanProgress>("get_scan_progress");
      setProgress(next);
      if (next.error) setError(next.error);
      if (!next.scanning && next.percentage === 100) {
        const nextPlan = await invoke<ResolutionPlan | null>("get_resolution_plan");
        setPlan(nextPlan);
      }
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function chooseSources(): Promise<void> {
    const chosen = await open({ directory: true, multiple: true, title: "Select LumaSift sources" });
    if (!chosen) return;
    setSources(Array.isArray(chosen) ? chosen : [chosen]);
  }

  function toggleType(type: SelectionType): void {
    setSelectedTypes((current) => current.includes(type)
      ? current.filter((item) => item !== type)
      : [...current, type]);
  }

  async function connectNas(): Promise<void> {
    setError(null);
    setBusy(true);
    try {
      await invoke("connect_nas_source", { request: nas });
      setSources((current) => current.includes(nas.uncPath) ? current : [...current, nas.uncPath]);
      setNas({ uncPath: "", username: "", password: "", rememberConnection: false });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function startScan(): Promise<void> {
    setError(null);
    setPlan(null);
    setBusy(true);
    try {
      const started = await invoke<ScanProgress>("start_resolution", {
        request: { sources, selectedTypes }
      });
      setProgress(started);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function cancelScan(): Promise<void> {
    try {
      await invoke("cancel_resolution");
      await refreshProgress();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function applyPlan(): Promise<void> {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      await invoke("apply_resolution_plan", { planId: plan.id });
      const refreshed = await invoke<ResolutionPlan | null>("get_resolution_plan");
      setPlan(refreshed);
      await refreshProgress();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function purgeQuarantine(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      await invoke("purge_quarantine", { confirmation: purgeText });
      setPurgeText("");
      await refreshProgress();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <header className="masthead">
        <div><p className="eyebrow">Cinematic duplicate resolution</p><h1>LumaSift</h1></div>
        <div className="proof-badge">SHA-256 exact proof<br /><span>Quarantine before purge</span></div>
      </header>

      <section className="hero-grid">
        <div className="hero-copy"><p className="eyebrow">Build 0.1.0 · Windows coordinator</p><h2>Keep the best exact copy. Recover every decision.</h2><p>Scan the sources you choose, prove exact identity before action, and route lower-ranked duplicates through an auditable quarantine.</p></div>
        <div className="progress-orb"><strong>{progress.percentage}%</strong><span>{progress.scanning ? "Scanning" : "Ready"}</span></div>
      </section>

      {error && <p className="error" role="alert">{error}</p>}

      <section className="workspace-grid">
        <section className="panel source-panel">
          <div className="section-heading"><div><p className="eyebrow">1 · Scope</p><h3>Sources & formats</h3></div><button className="secondary" onClick={() => void chooseSources()} disabled={busy}>Choose folders</button></div>
          <div className="format-row">
            {ALL_SELECTION_TYPES.map((type) => <label className="format-toggle" key={type}><input type="checkbox" checked={selectedTypes.includes(type)} onChange={() => toggleType(type)} /><span>{selectionLabel[type]}</span></label>)}
          </div>
          <div className="source-list">{sources.length === 0 ? <p>No sources selected yet.</p> : sources.map((source) => <div key={source}><span>{source}</span><button aria-label={`Remove ${source}`} onClick={() => setSources((current) => current.filter((item) => item !== source))}>×</button></div>)}</div>
          <div className="nas-block"><p className="eyebrow">Optional NAS connection</p><input value={nas.uncPath} placeholder="\\\\server\\share" onChange={(event) => setNas({ ...nas, uncPath: event.target.value })} /><input value={nas.username} placeholder="Domain\\username" autoComplete="username" onChange={(event) => setNas({ ...nas, username: event.target.value })} /><input value={nas.password} type="password" placeholder="Password" autoComplete="current-password" onChange={(event) => setNas({ ...nas, password: event.target.value })} /><label className="checkline"><input type="checkbox" checked={nas.rememberConnection} onChange={(event) => setNas({ ...nas, rememberConnection: event.target.checked })} />Remember Windows connection</label><button className="secondary" onClick={() => void connectNas()} disabled={busy || !nas.uncPath || !nas.username || !nas.password}>Connect NAS</button></div>
        </section>

        <section className="panel action-panel">
          <p className="eyebrow">2 · Proof plan</p><h3>Nothing changes during scanning.</h3><div className="progress-track"><div style={{ width: `${progress.percentage}%` }} /></div><p className="progress-copy"><strong>{progressText}</strong><br />{progress.message}</p><p className="current-file">{progress.currentPath ?? "Waiting for a selected source."}</p>
          <div className="action-row">{progress.scanning ? <button className="danger" onClick={() => void cancelScan()}>Cancel scan</button> : <button className="primary" onClick={() => void startScan()} disabled={busy || sources.length === 0 || selectedTypes.length === 0}>Build safe plan</button>}<button className="secondary" onClick={() => void refreshProgress()} disabled={busy}>Refresh</button></div>
        </section>
      </section>

      <section className="plan-section">
        <div className="section-heading"><div><p className="eyebrow">3 · Review & recovery</p><h3>{plan ? `${plan.groups.length} exact duplicate group${plan.groups.length === 1 ? "" : "s"}` : "No plan ready"}</h3></div>{plan && <div className="plan-metrics"><span>{formatBytes(plan.reclaimableBytes)} reclaimable</span><span>{plan.queuedFileCount} queued</span></div>}</div>
        {plan ? <>
          <div className="groups">{plan.groups.map((group) => <article className="group" key={group.id}><div className="group-head"><span>Exact SHA-256 group</span><strong>{formatBytes(group.reclaimableBytes)}</strong></div>{group.candidates.map((candidate) => <div className={candidateClass(candidate)} key={candidate.id}><div><strong>{candidate.displayName}</strong><p>{candidate.dispositionDetail}</p><small>{candidate.quality.reasons.join(" · ")}</small></div><span>{candidate.disposition.replaceAll("_", " ")}</span></div>)}</article>)}</div>
          <div className="quarantine-actions"><p>Applying this plan re-checks every selected candidate’s SHA-256 hash immediately before moving it. Retained files are never moved.</p><button className="primary" disabled={busy || plan.status !== "ready_for_review"} onClick={() => void applyPlan()}>Approve quarantine plan</button></div>
        </> : <p className="empty">A completed scan will show exact groups, rank evidence, and every planned disposition here.</p>}
      </section>

      {companion && <section className="companion-section"><div><p className="eyebrow">Mobile companion setup</p><h3>Configure Android & iOS through HTTPS</h3><p>Local coordinator: <code>{companion.localUrl}</code><br />Access token: <code>{companion.accessToken}</code><br />{companion.tlsNotice}</p></div></section>}

      <section className="purge-section"><div><p className="eyebrow">Separate irreversible action</p><h3>Purge only after recovery review</h3><p>Enter <code>ERASE LUMASIFT QUARANTINE</code> to permanently erase the LumaSift quarantine. This cannot be undone.</p></div><div className="purge-controls"><input value={purgeText} onChange={(event) => setPurgeText(event.target.value)} placeholder="Type confirmation" /><button className="danger" disabled={busy || purgeText !== "ERASE LUMASIFT QUARANTINE"} onClick={() => void purgeQuarantine()}>Permanently erase</button></div></section>
    </main>
  );
}
