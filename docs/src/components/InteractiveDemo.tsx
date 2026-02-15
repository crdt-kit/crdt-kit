import { useState, useRef, useCallback } from 'react';

type Tab = 'gc' | 'or' | 'lww';

export default function InteractiveDemo() {
  const [tab, setTab] = useState<Tab>('gc');
  return (
    <div className="bg-bg-card border border-border rounded-xl p-6 md:p-8 max-w-3xl mx-auto">
      <div className="flex gap-1 mb-6 border-b border-border pb-3">
        {(['gc', 'or', 'lww'] as Tab[]).map(id => (
          <button key={id} onClick={() => setTab(id)}
            className={`px-3.5 py-1.5 rounded-md text-sm font-semibold transition-all ${
              tab === id ? 'bg-teal/10 text-teal border border-teal/20' : 'text-text-dim border border-transparent hover:text-text'
            }`}>
            {{ gc: 'GCounter', or: 'ORSet', lww: 'LWWRegister' }[id]}
          </button>
        ))}
      </div>
      {tab === 'gc' && <GCounterDemo />}
      {tab === 'or' && <ORSetDemo />}
      {tab === 'lww' && <LWWDemo />}
    </div>
  );
}

// --- Shared ---
function Node({ name, value, detail, color, active }: {
  name: string; value: string; detail: string; color: string; active: boolean;
}) {
  return (
    <div className={`bg-bg border-2 rounded-xl px-6 py-4 text-center min-w-[140px] transition-all duration-300 ${
      active ? `border-${color} shadow-[0_0_24px_rgba(0,201,167,0.15)]` : 'border-border'
    }`}>
      <div className="font-mono text-xs text-text-dim mb-1">{name}</div>
      <div className={`text-2xl font-extrabold transition-all ${active ? `text-${color} scale-110` : 'text-text'}`}>{value}</div>
      <div className="font-mono text-[11px] text-text-dim mt-1">{detail}</div>
    </div>
  );
}

function Btn({ children, onClick, variant = 'default' }: {
  children: React.ReactNode; onClick: () => void; variant?: 'default' | 'merge' | 'reset';
}) {
  const cls = {
    default: 'border-border text-text hover:border-teal hover:text-teal',
    merge: 'bg-teal/10 text-teal border-teal/30 hover:bg-teal/20',
    reset: 'border-border text-text-dim hover:text-text',
  }[variant];
  return (
    <button onClick={onClick} className={`border rounded-md px-3 py-1.5 font-mono text-xs transition-all ${cls}`}>
      {children}
    </button>
  );
}

function Log({ lines }: { lines: string[] }) {
  const ref = useRef<HTMLDivElement>(null);
  return (
    <div ref={ref} className="bg-bg border border-border rounded-md p-3 font-mono text-[11px] text-text-dim max-h-32 overflow-y-auto leading-7"
      dangerouslySetInnerHTML={{ __html: lines.join('\n') }} />
  );
}

// --- GCounter ---
function GCounterDemo() {
  const [a, setA] = useState({ a: 0, b: 0 });
  const [b, setB] = useState({ a: 0, b: 0 });
  const [log, setLog] = useState<string[]>([]);
  const [flash, setFlash] = useState<'a' | 'b' | null>(null);

  const addLog = useCallback((msg: string) => setLog(prev => [...prev, msg]), []);

  const doFlash = (node: 'a' | 'b') => {
    setFlash(node);
    setTimeout(() => setFlash(null), 400);
  };

  const incA = () => { setA(s => ({ ...s, a: s.a + 1 })); doFlash('a'); addLog(`<span style="color:#00C9A7">node-a</span>.increment()  // ${a.a + 1 + a.b}`); };
  const incB = () => { setB(s => ({ ...s, b: s.b + 1 })); doFlash('b'); addLog(`<span style="color:#F7931A">node-b</span>.increment()  // ${b.a + b.b + 1}`); };

  const merge = () => {
    const ma = Math.max(a.a, b.a), mb = Math.max(a.b, b.b);
    setA({ a: ma, b: mb }); setB({ a: ma, b: mb });
    setFlash('a');
    setTimeout(() => setFlash('b'), 100);
    setTimeout(() => setFlash(null), 500);
    addLog(`<span style="color:#A259FF">merge!</span>  value = ${ma + mb}  <span style="color:#00C9A7">converged</span>`);
  };

  const reset = () => { setA({ a: 0, b: 0 }); setB({ a: 0, b: 0 }); setLog([]); };

  return (
    <div className="flex flex-col gap-5">
      <div className="flex justify-center gap-8 flex-wrap">
        <Node name="node-a" value={String(a.a + a.b)} detail={`{a:${a.a}, b:${a.b}}`} color="teal" active={flash === 'a'} />
        <Node name="node-b" value={String(b.a + b.b)} detail={`{a:${b.a}, b:${b.b}}`} color="orange" active={flash === 'b'} />
      </div>
      <div className="flex justify-center gap-2 flex-wrap">
        <Btn onClick={incA}>node-a.increment()</Btn>
        <Btn onClick={incB}>node-b.increment()</Btn>
        <Btn onClick={merge} variant="merge">merge()</Btn>
        <Btn onClick={reset} variant="reset">reset</Btn>
      </div>
      {log.length > 0 && <Log lines={log} />}
    </div>
  );
}

// --- ORSet ---
function ORSetDemo() {
  const items = ['milk', 'eggs', 'bread', 'butter', 'cheese', 'apple', 'rice'];
  const [aSet, setASet] = useState<Map<number, string>>(new Map());
  const [bSet, setBSet] = useState<Map<number, string>>(new Map());
  const [tag, setTag] = useState(1);
  const [idx, setIdx] = useState(0);
  const [log, setLog] = useState<string[]>([]);
  const [flash, setFlash] = useState<'a' | 'b' | null>(null);

  const addA = () => {
    const item = items[idx % items.length];
    const t = tag;
    setASet(new Map(aSet).set(t, item));
    setTag(t + 1); setIdx(idx + 1);
    setFlash('a'); setTimeout(() => setFlash(null), 400);
    setLog(l => [...l, `<span style="color:#00C9A7">node-a</span>.add("${item}")`]);
  };

  const addB = () => {
    const item = items[idx % items.length];
    const t = tag;
    setBSet(new Map(bSet).set(t, item));
    setTag(t + 1); setIdx(idx + 1);
    setFlash('b'); setTimeout(() => setFlash(null), 400);
    setLog(l => [...l, `<span style="color:#F7931A">node-b</span>.add("${item}")`]);
  };

  const removeA = () => {
    const keys = [...aSet.keys()];
    if (!keys.length) return;
    const k = keys[keys.length - 1];
    const item = aSet.get(k);
    const next = new Map(aSet); next.delete(k);
    setASet(next);
    setLog(l => [...l, `<span style="color:#00C9A7">node-a</span>.remove("${item}")`]);
  };

  const merge = () => {
    const merged = new Map([...aSet, ...bSet]);
    setASet(new Map(merged)); setBSet(new Map(merged));
    setFlash('a'); setTimeout(() => setFlash('b'), 100); setTimeout(() => setFlash(null), 500);
    const els = [...new Set(merged.values())].sort().join(', ');
    setLog(l => [...l, `<span style="color:#A259FF">merge!</span>  {${els}}  <span style="color:#00C9A7">converged</span>`]);
  };

  const reset = () => { setASet(new Map()); setBSet(new Map()); setTag(1); setIdx(0); setLog([]); };
  const elsA = [...new Set(aSet.values())].sort();
  const elsB = [...new Set(bSet.values())].sort();

  return (
    <div className="flex flex-col gap-5">
      <div className="flex justify-center gap-8 flex-wrap">
        <Node name="node-a" value={`{${elsA.join(', ')}}`} detail={`${elsA.length} elements`} color="teal" active={flash === 'a'} />
        <Node name="node-b" value={`{${elsB.join(', ')}}`} detail={`${elsB.length} elements`} color="orange" active={flash === 'b'} />
      </div>
      <div className="flex justify-center gap-2 flex-wrap">
        <Btn onClick={addA}>node-a.add()</Btn>
        <Btn onClick={addB}>node-b.add()</Btn>
        <Btn onClick={removeA}>node-a.remove()</Btn>
        <Btn onClick={merge} variant="merge">merge()</Btn>
        <Btn onClick={reset} variant="reset">reset</Btn>
      </div>
      {log.length > 0 && <Log lines={log} />}
    </div>
  );
}

// --- LWWRegister ---
function LWWDemo() {
  const names = ['"Alice"', '"Bob"', '"Charlie"', '"Diana"', '"Eve"'];
  const [a, setA] = useState({ val: '""', ts: 0 });
  const [b, setB] = useState({ val: '""', ts: 0 });
  const [clock, setClock] = useState(0);
  const [ni, setNi] = useState(0);
  const [log, setLog] = useState<string[]>([]);
  const [flash, setFlash] = useState<'a' | 'b' | null>(null);

  const setValA = () => {
    const c = clock + 1; const v = names[ni % names.length];
    setClock(c); setNi(ni + 1); setA({ val: v, ts: c });
    setFlash('a'); setTimeout(() => setFlash(null), 400);
    setLog(l => [...l, `<span style="color:#00C9A7">node-a</span>.set(${v})  ts=${c}`]);
  };

  const setValB = () => {
    const c = clock + 1; const v = names[ni % names.length];
    setClock(c); setNi(ni + 1); setB({ val: v, ts: c });
    setFlash('b'); setTimeout(() => setFlash(null), 400);
    setLog(l => [...l, `<span style="color:#F7931A">node-b</span>.set(${v})  ts=${c}`]);
  };

  const merge = () => {
    const winner = a.ts >= b.ts ? a : b;
    setA({ ...winner }); setB({ ...winner });
    setFlash('a'); setTimeout(() => setFlash('b'), 100); setTimeout(() => setFlash(null), 500);
    setLog(l => [...l, `<span style="color:#A259FF">merge!</span>  winner=${winner.val} (ts=${winner.ts})  <span style="color:#00C9A7">last writer wins</span>`]);
  };

  const reset = () => { setA({ val: '""', ts: 0 }); setB({ val: '""', ts: 0 }); setClock(0); setNi(0); setLog([]); };

  return (
    <div className="flex flex-col gap-5">
      <div className="flex justify-center gap-8 flex-wrap">
        <Node name="node-a" value={a.val} detail={`ts=${a.ts}`} color="teal" active={flash === 'a'} />
        <Node name="node-b" value={b.val} detail={`ts=${b.ts}`} color="orange" active={flash === 'b'} />
      </div>
      <div className="flex justify-center gap-2 flex-wrap">
        <Btn onClick={setValA}>node-a.set()</Btn>
        <Btn onClick={setValB}>node-b.set()</Btn>
        <Btn onClick={merge} variant="merge">merge()</Btn>
        <Btn onClick={reset} variant="reset">reset</Btn>
      </div>
      {log.length > 0 && <Log lines={log} />}
    </div>
  );
}
