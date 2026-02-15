import { useState, useRef, useEffect, useCallback } from 'react';

type Tab = 'chat' | 'counter' | 'cart';

export default function InteractiveDemo() {
  const [tab, setTab] = useState<Tab>('chat');
  const tabs: { id: Tab; icon: string; label: string }[] = [
    { id: 'chat', icon: '💬', label: 'Live Chat' },
    { id: 'counter', icon: '🔢', label: 'Shared Counter' },
    { id: 'cart', icon: '🛒', label: 'Shopping List' },
  ];

  return (
    <div className="max-w-3xl mx-auto">
      <div className="flex gap-1 mb-6 border-b border-border pb-3 overflow-x-auto">
        {tabs.map(t => (
          <button key={t.id} onClick={() => setTab(t.id)}
            className={`flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-semibold transition-all whitespace-nowrap ${
              tab === t.id
                ? 'bg-teal/10 text-teal border border-teal/20'
                : 'text-text-dim border border-transparent hover:text-text hover:bg-bg-card'
            }`}>
            <span>{t.icon}</span> {t.label}
          </button>
        ))}
      </div>
      {tab === 'chat' && <ChatDemo />}
      {tab === 'counter' && <CounterDemo />}
      {tab === 'cart' && <CartDemo />}
    </div>
  );
}

// ========== SHARED COMPONENTS ==========

function ActionBtn({ children, onClick, variant = 'default', disabled = false }: {
  children: React.ReactNode; onClick: () => void; variant?: 'default' | 'merge' | 'reset'; disabled?: boolean;
}) {
  const cls = {
    default: 'border-border text-text hover:border-teal hover:text-teal',
    merge: 'bg-teal/10 text-teal border-teal/30 hover:bg-teal/20',
    reset: 'border-border text-text-dim hover:text-text',
  }[variant];
  return (
    <button onClick={onClick} disabled={disabled}
      className={`border rounded-lg px-4 py-2 font-mono text-xs transition-all disabled:opacity-40 disabled:cursor-not-allowed ${cls}`}>
      {children}
    </button>
  );
}

function ConvergedBanner({ text }: { text: string }) {
  return (
    <div className="inline-flex items-center gap-2 bg-teal/10 border border-teal/20 rounded-lg px-4 py-2 text-teal text-xs font-semibold">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
      {text}
    </div>
  );
}

// ========== 1. LIVE CHAT (ORSet of messages) ==========

interface Message { id: string; author: 'alice' | 'bob'; text: string; ts: number; }

const MSGS_A = ["Hey! How's the project going?", "I pushed the new feature 🚀", "Let's sync up tomorrow", "Found the bug, fixing now", "All tests passing ✓"];
const MSGS_B = ["Going great! Almost done", "Nice, I'll review it", "Sounds good, morning works", "Awesome, I'll pull the fix", "Ship it! 🎉"];

function ChatDemo() {
  const [aMsg, setAMsg] = useState<Message[]>([]);
  const [bMsg, setBMsg] = useState<Message[]>([]);
  const [aNet, setANet] = useState<'offline' | 'online'>('offline');
  const [bNet, setBNet] = useState<'offline' | 'online'>('offline');
  const [aIdx, setAIdx] = useState(0);
  const [bIdx, setBIdx] = useState(0);
  const [clock, setClock] = useState(0);
  const [syncing, setSyncing] = useState(false);
  const [synced, setSynced] = useState(false);
  const aRef = useRef<HTMLDivElement>(null);
  const bRef = useRef<HTMLDivElement>(null);
  const scroll = useCallback((r: React.RefObject<HTMLDivElement | null>) => { r.current && (r.current.scrollTop = r.current.scrollHeight); }, []);
  useEffect(() => { scroll(aRef); }, [aMsg, scroll]);
  useEffect(() => { scroll(bRef); }, [bMsg, scroll]);

  const sendA = () => { const ts = clock + 1; setAMsg(p => [...p, { id: `a-${ts}`, author: 'alice', text: MSGS_A[aIdx % MSGS_A.length], ts }]); setClock(ts); setAIdx(i => i + 1); setSynced(false); };
  const sendB = () => { const ts = clock + 1; setBMsg(p => [...p, { id: `b-${ts}`, author: 'bob', text: MSGS_B[bIdx % MSGS_B.length], ts }]); setClock(ts); setBIdx(i => i + 1); setSynced(false); };

  const merge = () => {
    setSyncing(true); setANet('online'); setBNet('online'); setSynced(false);
    setTimeout(() => {
      const m = new Map<string, Message>(); aMsg.forEach(x => m.set(x.id, x)); bMsg.forEach(x => m.set(x.id, x));
      const merged = [...m.values()].sort((a, b) => a.ts - b.ts);
      setAMsg(merged); setBMsg(merged); setSyncing(false); setSynced(true);
      setTimeout(() => { setANet('offline'); setBNet('offline'); }, 2000);
    }, 800);
  };

  const reset = () => { setAMsg([]); setBMsg([]); setANet('offline'); setBNet('offline'); setAIdx(0); setBIdx(0); setClock(0); setSyncing(false); setSynced(false); };
  const aOnly = aMsg.filter(m => !bMsg.some(b => b.id === m.id)).length;
  const bOnly = bMsg.filter(m => !aMsg.some(a => a.id === m.id)).length;
  const conv = synced && aMsg.length === bMsg.length && aMsg.every((m, i) => m.id === bMsg[i]?.id);

  return (
    <div>
      <div className="grid md:grid-cols-2 gap-4 mb-5">
        <ChatDevice name="Alice" emoji="📱" color="teal" msgs={aMsg} net={aNet} ref={aRef} pending={aOnly} syncing={syncing} conv={conv} side="alice" />
        <ChatDevice name="Bob" emoji="💻" color="orange" msgs={bMsg} net={bNet} ref={bRef} pending={bOnly} syncing={syncing} conv={conv} side="bob" />
      </div>
      <div className="flex justify-center gap-2 flex-wrap mb-4">
        <ActionBtn onClick={sendA} disabled={syncing}><span className="text-teal">Alice</span> sends</ActionBtn>
        <ActionBtn onClick={sendB} disabled={syncing}><span className="text-orange">Bob</span> sends</ActionBtn>
        <ActionBtn onClick={merge} variant="merge" disabled={syncing || (aMsg.length === 0 && bMsg.length === 0)}>{syncing ? 'Syncing...' : 'Sync / Merge'}</ActionBtn>
        <ActionBtn onClick={reset} variant="reset" disabled={syncing}>Reset</ActionBtn>
      </div>
      <div className="text-center">
        {conv && <ConvergedBanner text="Both devices converged — same messages, same order. No conflicts." />}
        {!conv && (aOnly > 0 || bOnly > 0) && (
          <p className="text-text-dim text-xs">
            {aOnly > 0 && <span className="text-teal">{aOnly} only on Alice</span>}
            {aOnly > 0 && bOnly > 0 && ' · '}
            {bOnly > 0 && <span className="text-orange">{bOnly} only on Bob</span>}
            <span className="ml-2">— press Sync to merge</span>
          </p>
        )}
      </div>
    </div>
  );
}

const ChatDevice = ({ name, emoji, color, msgs, net, ref, pending, syncing, conv, side }: {
  name: string; emoji: string; color: 'teal' | 'orange'; msgs: Message[]; net: string; ref: React.RefObject<HTMLDivElement | null>; pending: number; syncing: boolean; conv: boolean; side: 'alice' | 'bob';
}) => {
  const bc = conv ? (color === 'teal' ? 'border-teal' : 'border-orange') : (color === 'teal' ? 'border-teal/30' : 'border-orange/30');
  const tc = color === 'teal' ? 'text-teal' : 'text-orange';
  return (
    <div className={`bg-bg-card border-2 rounded-xl overflow-hidden transition-all duration-500 ${bc}`}>
      <div className="flex items-center gap-2 px-4 py-2.5 border-b border-border">
        <span className="text-base">{emoji}</span>
        <span className={`font-bold text-sm ${tc}`}>{name}</span>
        <span className="ml-auto flex items-center gap-1.5 text-[10px] font-mono">
          <span className={`w-1.5 h-1.5 rounded-full ${net === 'online' ? 'bg-green' : 'bg-text-dim'} ${syncing ? 'animate-pulse' : ''}`}></span>
          <span className="text-text-dim">{net === 'online' ? (syncing ? 'syncing' : 'online') : 'offline'}</span>
        </span>
        {pending > 0 && !conv && <span className={`${tc} text-[10px] font-bold bg-bg rounded-full px-1.5 py-0.5`}>{pending} pending</span>}
      </div>
      <div ref={ref} className="h-48 overflow-y-auto p-3 space-y-2" style={{ scrollbarWidth: 'thin' }}>
        {msgs.length === 0 && <div className="h-full flex items-center justify-center text-text-dim text-xs italic">No messages yet</div>}
        {msgs.map(m => {
          const own = m.author === side;
          return (
            <div key={m.id} className={`flex ${own ? 'justify-end' : 'justify-start'}`}>
              <div className={`max-w-[85%] rounded-xl px-3 py-2 text-xs leading-relaxed ${own ? 'bg-teal/10 border border-teal/10' : 'bg-orange/10 border border-orange/10'}`}>
                <div className={`text-[9px] font-bold mb-0.5 ${m.author === 'alice' ? 'text-teal' : 'text-orange'}`}>{m.author === 'alice' ? 'Alice' : 'Bob'}</div>
                {m.text}<span className="text-text-dim text-[8px] ml-2">ts:{m.ts}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

// ========== 2. SHARED COUNTER (GCounter) ==========

function CounterDemo() {
  const [a, setA] = useState({ a: 0, b: 0 });
  const [b, setB] = useState({ a: 0, b: 0 });
  const [flash, setFlash] = useState<'a' | 'b' | null>(null);
  const [synced, setSynced] = useState(false);

  const doFlash = (n: 'a' | 'b') => { setFlash(n); setTimeout(() => setFlash(null), 400); };
  const incA = () => { setA(s => ({ ...s, a: s.a + 1 })); doFlash('a'); setSynced(false); };
  const incB = () => { setB(s => ({ ...s, b: s.b + 1 })); doFlash('b'); setSynced(false); };
  const inc5A = () => { setA(s => ({ ...s, a: s.a + 5 })); doFlash('a'); setSynced(false); };
  const inc5B = () => { setB(s => ({ ...s, b: s.b + 5 })); doFlash('b'); setSynced(false); };

  const merge = () => {
    const ma = Math.max(a.a, b.a), mb = Math.max(a.b, b.b);
    setA({ a: ma, b: mb }); setB({ a: ma, b: mb });
    doFlash('a'); setTimeout(() => doFlash('b'), 100);
    setSynced(true);
  };

  const reset = () => { setA({ a: 0, b: 0 }); setB({ a: 0, b: 0 }); setFlash(null); setSynced(false); };
  const conv = synced && (a.a + a.b) === (b.a + b.b) && a.a === b.a && a.b === b.b;

  return (
    <div>
      <p className="text-text-dim text-xs text-center mb-5">
        GCounter: each device has its own slot. On merge, take the <code className="text-teal">max()</code> of each slot. The total is the sum.
      </p>
      <div className="grid md:grid-cols-2 gap-4 mb-5">
        <CounterDevice name="Device A" color="teal" local={a.a} remote={a.b} total={a.a + a.b} active={flash === 'a'} conv={conv} slotLabels={['a', 'b']} />
        <CounterDevice name="Device B" color="orange" local={b.b} remote={b.a} total={b.a + b.b} active={flash === 'b'} conv={conv} slotLabels={['a', 'b']} />
      </div>

      {/* State vectors */}
      <div className="grid md:grid-cols-2 gap-4 mb-5">
        <div className="bg-bg border border-border rounded-lg px-4 py-2 font-mono text-xs text-center">
          <span className="text-text-dim">state: </span>
          <span className="text-teal">{`{ a: ${a.a}, b: ${a.b} }`}</span>
          <span className="text-text-dim"> = </span>
          <span className="text-text font-bold">{a.a + a.b}</span>
        </div>
        <div className="bg-bg border border-border rounded-lg px-4 py-2 font-mono text-xs text-center">
          <span className="text-text-dim">state: </span>
          <span className="text-orange">{`{ a: ${b.a}, b: ${b.b} }`}</span>
          <span className="text-text-dim"> = </span>
          <span className="text-text font-bold">{b.a + b.b}</span>
        </div>
      </div>

      <div className="flex justify-center gap-2 flex-wrap mb-4">
        <ActionBtn onClick={incA}><span className="text-teal">A</span> +1</ActionBtn>
        <ActionBtn onClick={inc5A}><span className="text-teal">A</span> +5</ActionBtn>
        <ActionBtn onClick={incB}><span className="text-orange">B</span> +1</ActionBtn>
        <ActionBtn onClick={inc5B}><span className="text-orange">B</span> +5</ActionBtn>
        <ActionBtn onClick={merge} variant="merge">merge()</ActionBtn>
        <ActionBtn onClick={reset} variant="reset">Reset</ActionBtn>
      </div>
      <div className="text-center">
        {conv && <ConvergedBanner text="Converged! Both devices agree on the count. max() per slot ensures no increment is lost." />}
        {!conv && (a.a + a.b) !== (b.a + b.b) && (
          <p className="text-text-dim text-xs">
            Diverged: <span className="text-teal">{a.a + a.b}</span> vs <span className="text-orange">{b.a + b.b}</span> — press merge()
          </p>
        )}
      </div>
    </div>
  );
}

function CounterDevice({ name, color, local, remote, total, active, conv, slotLabels }: {
  name: string; color: 'teal' | 'orange'; local: number; remote: number; total: number; active: boolean; conv: boolean; slotLabels: string[];
}) {
  const bc = conv ? (color === 'teal' ? 'border-teal' : 'border-orange') : active ? (color === 'teal' ? 'border-teal' : 'border-orange') : 'border-border';
  const tc = color === 'teal' ? 'text-teal' : 'text-orange';
  const mySlot = color === 'teal' ? slotLabels[0] : slotLabels[1];
  return (
    <div className={`bg-bg-card border-2 rounded-xl p-5 text-center transition-all duration-300 ${bc}`}>
      <div className={`font-mono text-xs ${tc} mb-2 font-bold`}>{name}</div>
      <div className={`text-4xl font-extrabold transition-all duration-300 ${active ? `${tc} scale-110` : 'text-text'}`}>{total}</div>
      <div className="text-text-dim text-[10px] font-mono mt-2">
        my slot [{mySlot}]: <span className={tc}>{local}</span> · remote: {remote}
      </div>
    </div>
  );
}

// ========== 3. SHOPPING LIST (ORSet) ==========

const ITEMS = ['🥛 Milk', '🥚 Eggs', '🍞 Bread', '🧈 Butter', '🧀 Cheese', '🍎 Apple', '🍚 Rice', '☕ Coffee'];

function CartDemo() {
  const [aSet, setASet] = useState<Map<number, string>>(new Map());
  const [bSet, setBSet] = useState<Map<number, string>>(new Map());
  const [tag, setTag] = useState(1);
  const [idx, setIdx] = useState(0);
  const [synced, setSynced] = useState(false);

  const addA = () => { const t = tag; const item = ITEMS[idx % ITEMS.length]; setASet(new Map(aSet).set(t, item)); setTag(t + 1); setIdx(i => i + 1); setSynced(false); };
  const addB = () => { const t = tag; const item = ITEMS[idx % ITEMS.length]; setBSet(new Map(bSet).set(t, item)); setTag(t + 1); setIdx(i => i + 1); setSynced(false); };

  const removeLastA = () => { const keys = [...aSet.keys()]; if (!keys.length) return; const k = keys[keys.length - 1]; const n = new Map(aSet); n.delete(k); setASet(n); setSynced(false); };
  const removeLastB = () => { const keys = [...bSet.keys()]; if (!keys.length) return; const k = keys[keys.length - 1]; const n = new Map(bSet); n.delete(k); setBSet(n); setSynced(false); };

  const merge = () => {
    const merged = new Map([...aSet, ...bSet]);
    setASet(new Map(merged)); setBSet(new Map(merged)); setSynced(true);
  };

  const reset = () => { setASet(new Map()); setBSet(new Map()); setTag(1); setIdx(0); setSynced(false); };
  const elsA = [...new Set(aSet.values())];
  const elsB = [...new Set(bSet.values())];
  const conv = synced && elsA.length === elsB.length && elsA.every((v, i) => v === elsB[i]);

  return (
    <div>
      <p className="text-text-dim text-xs text-center mb-5">
        ORSet (Observed-Remove Set): add and remove items freely. On merge, <code className="text-teal">add wins</code> over concurrent remove.
      </p>
      <div className="grid md:grid-cols-2 gap-4 mb-5">
        <CartDevice name="Alice's List" color="teal" items={elsA} conv={conv} />
        <CartDevice name="Bob's List" color="orange" items={elsB} conv={conv} />
      </div>
      <div className="flex justify-center gap-2 flex-wrap mb-4">
        <ActionBtn onClick={addA}><span className="text-teal">Alice</span> adds</ActionBtn>
        <ActionBtn onClick={removeLastA}><span className="text-teal">Alice</span> removes</ActionBtn>
        <ActionBtn onClick={addB}><span className="text-orange">Bob</span> adds</ActionBtn>
        <ActionBtn onClick={removeLastB}><span className="text-orange">Bob</span> removes</ActionBtn>
        <ActionBtn onClick={merge} variant="merge">merge()</ActionBtn>
        <ActionBtn onClick={reset} variant="reset">Reset</ActionBtn>
      </div>
      <div className="text-center">
        {conv && <ConvergedBanner text="Converged! Both lists have the same items. Add-wins semantics: no item is accidentally lost." />}
        {!conv && (elsA.length > 0 || elsB.length > 0) && (
          <p className="text-text-dim text-xs">
            <span className="text-teal">{elsA.length} items on Alice</span>
            {' · '}
            <span className="text-orange">{elsB.length} items on Bob</span>
            <span className="ml-2">— press merge()</span>
          </p>
        )}
      </div>
    </div>
  );
}

function CartDevice({ name, color, items, conv }: { name: string; color: 'teal' | 'orange'; items: string[]; conv: boolean }) {
  const bc = conv ? (color === 'teal' ? 'border-teal' : 'border-orange') : (color === 'teal' ? 'border-teal/30' : 'border-orange/30');
  const tc = color === 'teal' ? 'text-teal' : 'text-orange';
  return (
    <div className={`bg-bg-card border-2 rounded-xl overflow-hidden transition-all duration-500 ${bc}`}>
      <div className="flex items-center gap-2 px-4 py-2.5 border-b border-border">
        <span className="text-base">🛒</span>
        <span className={`font-bold text-sm ${tc}`}>{name}</span>
        <span className="ml-auto text-text-dim text-[10px] font-mono">{items.length} items</span>
      </div>
      <div className="h-40 overflow-y-auto p-3" style={{ scrollbarWidth: 'thin' }}>
        {items.length === 0 && <div className="h-full flex items-center justify-center text-text-dim text-xs italic">Empty list</div>}
        <div className="space-y-1.5">
          {items.map((item, i) => (
            <div key={`${item}-${i}`} className="flex items-center gap-2 bg-bg border border-border rounded-lg px-3 py-1.5 text-xs">
              <span>{item}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
