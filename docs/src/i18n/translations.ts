export const languages = { en: 'English', es: 'Español' } as const;
export type Lang = keyof typeof languages;

const en = {
  'nav.features': 'Features',
  'nav.types': 'Types',
  'nav.start': 'Quick Start',
  'nav.arch': 'Architecture',
  'nav.ecosystem': 'Ecosystem',
  'nav.docs': 'Docs',

  'hero.tagline': 'Conflict-Free Replicated Data Types for Rust',
  'hero.sub': 'Lightweight (~50KB), no_std compatible, optimized for IoT, edge computing, WASM, and local-first architectures.',
  'hero.getstarted': 'Get Started',
  'hero.apidocs': 'API Docs',

  'anim.device_a': 'Phone',
  'anim.device_b': 'Laptop',
  'anim.device_c': 'IoT Sensor',
  'anim.offline': 'offline',
  'anim.editing': 'editing locally...',
  'anim.syncing': 'syncing...',
  'anim.converged': 'converged!',

  'what.title': 'What are CRDTs?',
  'what.p1': 'Conflict-Free Replicated Data Types are data structures that can be replicated across multiple devices, updated independently and concurrently, and merged automatically without conflicts.',
  'what.p2': 'Unlike traditional databases that require a central server to coordinate writes, CRDTs guarantee that all replicas will converge to the same state regardless of the order in which updates are received.',
  'what.p3': 'This makes them ideal for offline-first apps, peer-to-peer systems, IoT networks, and any scenario where devices need to work independently and sync later.',

  'why.title': 'Why crdt-kit?',
  'why.sub': 'Built for resource-constrained, latency-sensitive environments where existing solutions add too much overhead.',
  'why.s1': 'Binary size',
  'why.s2': 'Dependencies',
  'why.s3': 'CRDT types',
  'why.s4': 'Tests',
  'why.s5': 'ops/sec peak',

  'feat.title': 'Features',
  'feat.nostd': 'Runs on bare metal, Raspberry Pi, ESP32. All types work with #![no_std] + alloc.',
  'feat.delta': 'Only send what changed. DeltaCrdt trait for GCounter, PNCounter, ORSet. Minimizes bandwidth on LoRa, BLE.',
  'feat.wasm': 'First-class wasm-bindgen bindings. Same CRDTs in browser, Deno, Node.js, and Rust backend.',
  'feat.migrate': 'Transparent, lazy migrations on read. #[crdt_schema] + #[migration] proc macros. Deterministic.',
  'feat.storage': 'Three backends: SQLite (bundled), redb (pure Rust), memory. Event sourcing, snapshots, compaction.',
  'feat.codegen': 'Define entities in TOML, run crdt generate. Get models, migrations, repositories, events, sync.',
  'feat.serde': 'Serialize/Deserialize for all 9 CRDT types. JSON, MessagePack, Postcard, CBOR — any serde format.',
  'feat.events': 'Full event log with append, replay, snapshots, compaction. EventStore trait on all backends.',
  'feat.devtools': 'CLI: status, inspect, compact, export, generate, dev-ui. Web panel for visual inspection.',

  'compare.title': 'How It Compares',
  'crdts.title': '9 CRDT Types',
  'crdts.sub': 'From counters to collaborative text. Every type is Send + Sync, serde-ready, mathematically convergent.',

  'qs.title': 'Quick Start',
  'arch.title': 'Architecture',
  'arch.sub': 'Multi-crate workspace. Each crate is independently versioned. Use only what you need.',
  'arch.s1': 'Define',
  'arch.s1d': 'Write a crdt-schema.toml with entities, versions, CRDT fields, and relations.',
  'arch.s2': 'Generate',
  'arch.s2d': 'Run crdt generate. Get models, migrations, repository traits, events, sync.',
  'arch.s3': 'Use',
  'arch.s3d': 'Import Persistence<S> in your app. Access repos, store data, sync between nodes.',

  'use.title': 'Use Cases',
  'use.iot': 'IoT & Sensors',
  'use.iotd': 'no_std core on ESP32, Raspberry Pi. Delta sync over LoRa/BLE. Schema migrations handle OTA updates.',
  'use.mobile': 'Mobile Apps',
  'use.mobiled': 'Offline-first. Edit without network. Changes merge automatically on reconnect. No conflict dialogs.',
  'use.collab': 'Real-time Collaboration',
  'use.collabd': 'TextCrdt for docs-style editing. ORSet for shared collections. No central coordinator needed.',
  'use.edge': 'Edge Computing',
  'use.edged': 'CRDTs at CDN edges. Local writes, delta sync between nodes. Pure-Rust redb — no C deps.',
  'use.p2p': 'P2P Networks',
  'use.p2pd': 'No server. Every peer is equal. Any transport: WebSocket, WebRTC, Bluetooth. Order doesn\'t matter.',
  'use.wasmuc': 'WASM & Browser',
  'use.wasmd': 'Same logic in Rust backend and browser. wasm-bindgen bindings. Ship one codebase everywhere.',

  'eco.title': 'Ecosystem',
  'eco.sub': '7 crates, independently versioned on crates.io. Use only what you need.',

  'demo.title': 'Interactive Demo',
  'demo.sub': 'See CRDTs in action. Click buttons to simulate operations on distributed nodes.',

  'perf.title': 'Performance',
  'perf.sub': 'Measured with Criterion on optimized builds.',

  'guar.title': 'Mathematical Guarantees',
  'guar.sub': 'All CRDTs satisfy Strong Eventual Consistency (SEC). Verified by 268 tests.',
  'guar.comm': 'Commutativity',
  'guar.commd': 'Order of sync doesn\'t matter.',
  'guar.assoc': 'Associativity',
  'guar.assocd': 'Group syncs however you want.',
  'guar.idem': 'Idempotency',
  'guar.idemd': 'Safe to retry. No duplicates.',

  'cli.title': 'Developer CLI',
  'cta.title': 'Start building offline-first',
  'footer.guide': 'Dev Guide',
} as const;

const es: typeof en = {
  'nav.features': 'Funcionalidades',
  'nav.types': 'Tipos',
  'nav.start': 'Inicio',
  'nav.arch': 'Arquitectura',
  'nav.ecosystem': 'Ecosistema',
  'nav.docs': 'Docs',

  'hero.tagline': 'Tipos de Datos Replicados Libres de Conflictos para Rust',
  'hero.sub': 'Ligero (~50KB), compatible con no_std, optimizado para IoT, edge computing, WASM y arquitecturas local-first.',
  'hero.getstarted': 'Comenzar',
  'hero.apidocs': 'Documentación API',

  'anim.device_a': 'Teléfono',
  'anim.device_b': 'Laptop',
  'anim.device_c': 'Sensor IoT',
  'anim.offline': 'sin conexión',
  'anim.editing': 'editando local...',
  'anim.syncing': 'sincronizando...',
  'anim.converged': '¡convergido!',

  'what.title': '¿Qué son los CRDTs?',
  'what.p1': 'Los Conflict-Free Replicated Data Types son estructuras de datos que pueden replicarse en múltiples dispositivos, actualizarse de forma independiente y concurrente, y fusionarse automáticamente sin conflictos.',
  'what.p2': 'A diferencia de las bases de datos tradicionales que requieren un servidor central, los CRDTs garantizan que todas las réplicas convergerán al mismo estado sin importar el orden de las actualizaciones.',
  'what.p3': 'Esto los hace ideales para apps offline-first, sistemas P2P, redes IoT y cualquier escenario donde los dispositivos necesiten trabajar independientemente.',

  'why.title': '¿Por qué crdt-kit?',
  'why.sub': 'Construido para entornos con recursos limitados donde las soluciones existentes agregan demasiado overhead.',
  'why.s1': 'Tamaño binario',
  'why.s2': 'Dependencias',
  'why.s3': 'Tipos CRDT',
  'why.s4': 'Tests',
  'why.s5': 'ops/seg pico',

  'feat.title': 'Funcionalidades',
  'feat.nostd': 'Corre en bare metal, Raspberry Pi, ESP32. Todos los tipos funcionan con #![no_std] + alloc.',
  'feat.delta': 'Solo envía lo que cambió. Trait DeltaCrdt para GCounter, PNCounter, ORSet. Minimiza ancho de banda.',
  'feat.wasm': 'Bindings wasm-bindgen de primera clase. Mismos CRDTs en navegador, Deno, Node.js y backend Rust.',
  'feat.migrate': 'Migraciones transparentes y lazy al leer. Proc macros #[crdt_schema] + #[migration]. Determinístico.',
  'feat.storage': 'Tres backends: SQLite (bundled), redb (Rust puro), memoria. Event sourcing, snapshots, compactación.',
  'feat.codegen': 'Define entidades en TOML, ejecuta crdt generate. Obtén modelos, migraciones, repositorios, eventos, sync.',
  'feat.serde': 'Serialize/Deserialize para los 9 tipos CRDT. JSON, MessagePack, Postcard, CBOR — cualquier formato serde.',
  'feat.events': 'Log de eventos completo con append, replay, snapshots, compactación. Trait EventStore en todos los backends.',
  'feat.devtools': 'CLI: status, inspect, compact, export, generate, dev-ui. Panel web para inspección visual.',

  'compare.title': 'Comparación',
  'crdts.title': '9 Tipos de CRDT',
  'crdts.sub': 'Desde contadores hasta texto colaborativo. Todos Send + Sync, serde-ready, convergentes.',

  'qs.title': 'Inicio Rápido',
  'arch.title': 'Arquitectura',
  'arch.sub': 'Workspace multi-crate. Cada crate versionado independientemente. Usa solo lo que necesites.',
  'arch.s1': 'Definir',
  'arch.s1d': 'Escribe un crdt-schema.toml con entidades, versiones, campos CRDT y relaciones.',
  'arch.s2': 'Generar',
  'arch.s2d': 'Ejecuta crdt generate. Obtén modelos, migraciones, traits de repositorio, eventos, sync.',
  'arch.s3': 'Usar',
  'arch.s3d': 'Importa Persistence<S> en tu app. Accede a repositorios, almacena datos, sincroniza.',

  'use.title': 'Casos de Uso',
  'use.iot': 'IoT y Sensores',
  'use.iotd': 'Core no_std en ESP32, Raspberry Pi. Delta sync por LoRa/BLE. Migraciones manejan OTA.',
  'use.mobile': 'Apps Móviles',
  'use.mobiled': 'Offline-first. Edita sin red. Los cambios se fusionan automáticamente al reconectar.',
  'use.collab': 'Colaboración en Tiempo Real',
  'use.collabd': 'TextCrdt para edición estilo Google Docs. ORSet para colecciones. Sin coordinador central.',
  'use.edge': 'Edge Computing',
  'use.edged': 'CRDTs en nodos edge de CDN. Escrituras locales, delta sync. Backend redb puro Rust.',
  'use.p2p': 'Redes P2P',
  'use.p2pd': 'Sin servidor. Cada peer es igual. Cualquier transporte: WebSocket, WebRTC, Bluetooth.',
  'use.wasmuc': 'WASM y Navegador',
  'use.wasmd': 'Misma lógica en backend Rust y navegador. Bindings wasm-bindgen. Un solo codebase.',

  'eco.title': 'Ecosistema',
  'eco.sub': '7 crates, versionados independientemente en crates.io. Usa solo lo que necesites.',

  'demo.title': 'Demo Interactivo',
  'demo.sub': 'Mira los CRDTs en acción. Haz clic para simular operaciones en nodos distribuidos.',

  'perf.title': 'Rendimiento',
  'perf.sub': 'Medido con Criterion en builds optimizados.',

  'guar.title': 'Garantías Matemáticas',
  'guar.sub': 'Todos los CRDTs satisfacen Strong Eventual Consistency (SEC). Verificado por 268 tests.',
  'guar.comm': 'Conmutatividad',
  'guar.commd': 'El orden de sync no importa.',
  'guar.assoc': 'Asociatividad',
  'guar.assocd': 'Agrupa los syncs como quieras.',
  'guar.idem': 'Idempotencia',
  'guar.idemd': 'Seguro de reintentar. Sin duplicados.',

  'cli.title': 'CLI para Desarrolladores',
  'cta.title': 'Empieza a construir offline-first',
  'footer.guide': 'Guía Dev',
};

export const translations = { en, es } as const;

export function t(lang: Lang, key: keyof typeof en): string {
  return translations[lang][key] ?? translations.en[key] ?? key;
}

export function getLangFromUrl(url: URL): Lang {
  const [, lang] = url.pathname.split('/');
  if (lang === 'es') return 'es';
  return 'en';
}

export function getOtherLang(lang: Lang): Lang {
  return lang === 'en' ? 'es' : 'en';
}
