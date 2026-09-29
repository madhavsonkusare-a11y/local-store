import { readFileSync } from 'node:fs';
export const catalog = [
 { name:'Actual Budget', source_url:'https://actualbudget.org', description:'A privacy-focused app for managing your finances.', category:'Money, Budgeting & Management', license:'MIT', icon:null, warning:false, capability:'connect' },
 { name:'Immich', source_url:'https://immich.app', description:'Self-hosted photo and video management solution.', category:'Photo Galleries', license:'AGPL-3.0', icon:null, warning:false, capability:'connect' },
 { name:'Memos', source_url:'https://usememos.com', description:'A lightweight, self-hosted memo hub.', category:'Note-taking & Editors', license:'MIT', icon:null, warning:false, capability:'preview_install', recipe_id:'memos' },
 { name:'n8n', source_url:'https://github.com/n8n-io/n8n', description:'Workflow automation platform.', category:'Automation', license:'Sustainable Use License', icon:null, warning:false, capability:'preview_install', recipe_id:'n8n' },
 { name:'Uptime Kuma', source_url:'https://github.com/louislam/uptime-kuma', description:'A friendly monitoring tool.', category:'Status / Uptime pages', license:'MIT', icon:null, warning:true, capability:'preview_install', recipe_id:'uptime-kuma' }
];
export const apps = [{ id:'studio-notes', display_name:'Studio notes', launch_url:'http://localhost:5230', icon_path:null, runtime:{kind:'external'}, status:'connected', created_at_unix:1, updated_at_unix:1, catalog_id:null }];
const recipes = Object.fromEntries(['memos', 'n8n', 'uptime-kuma'].map(id =>
 [id, JSON.parse(readFileSync(new URL(`../../src/recipes/${id}.json`, import.meta.url), 'utf8'))]
));
// No reviewed recipe declares typed setup yet, so the projection the backend
// would return for one is described here. Shape only: the real values come
// from PlanTemplate::setup_review, which has its own tests.
export const setupReview = {
 service_count: 2,
 generated_credential_count: 1,
 fields: [
  { key:'SITE_NAME', label:'Site name', required:true, sensitive:false, has_default:false, control:'text', server_validated:true },
  { key:'ADMIN_EMAIL', label:'Administrator email', required:false, sensitive:false, has_default:true, default:'admin@example.com', control:'text', server_validated:true },
  { key:'API_KEY', label:'Upstream API key', required:false, sensitive:true, has_default:true, control:'password', server_validated:true },
  { key:'MAX_UPLOADS', label:'Maximum uploads', required:false, sensitive:false, has_default:true, default:'20', control:'number', server_validated:true, min:1, max:100 },
  { key:'ALLOW_SIGNUPS', label:'Allow sign-ups', required:false, sensitive:false, has_default:true, default:'false', control:'boolean', server_validated:true },
  { key:'THEME', label:'Theme', required:false, sensitive:false, has_default:true, default:'dark', control:'choice', server_validated:true, options:['dark','light'] }
 ]
};

export function installAdapter(page, options = {}) {
 const fixtureCatalog = options.catalog ?? catalog, fixtureApps = options.apps ?? apps;
 // Attach the projection to one recipe when a test asks for it, leaving the
 // others exactly as they were.
 const withSetup = structuredClone(recipes);
 if (options.setup) withSetup[options.setup].setup_review = setupReview;
 return page.addInitScript(({catalog, apps, failure, recipes}) => {
   let current = structuredClone(apps);
   window.__calls = [];
   window.__TAURI__ = { core: { invoke: async (command, args = {}) => {
     window.__calls.push({ command, args }); if (failure === command) throw new Error(`Could not ${command.replaceAll('_',' ')}`);
     if (command === 'search_catalog') {
       const filters = args.filters || {}, query = args.query.toLowerCase();
       const terms = {writing:['note-taking','wiki','document','office'],automation:['automation','workflow','integration'],media:['photo','media','video','audio','ebook'],developer:['development','developer','source code','ide','api']};
       const base = catalog.filter(a => (`${a.name} ${a.description} ${(a.aliases || []).join(' ')}`).toLowerCase().includes(query)
         && (!filters.capability || a.capability === filters.capability || (filters.capability === 'connect' && a.web_ui))
         && (!filters.license || (a.licenses || [a.license]).includes(filters.license))
         && (!filters.architecture || (a.architectures || []).includes(filters.architecture))
         && (!filters.hide_warnings || !a.warning)
         && (!filters.collection || (terms[filters.collection] || []).some(term => `${a.category} ${(a.tags || []).join(' ')}`.toLowerCase().includes(term))));
       const filtered = base.filter(a => !args.category || a.category === args.category);
       const counts = {}; for (const a of base) counts[a.category] = (counts[a.category] || 0) + 1;
       return { entries: filtered.slice(args.offset, args.offset + args.limit), total: filtered.length, catalog_total: catalog.length, offset: args.offset, limit: args.limit,
         categories: [...new Set(catalog.map(a=>a.category))].sort(), category_counts: Object.entries(counts).map(([value,count])=>({value,count})),
         licenses: [...new Set(catalog.flatMap(a=>a.licenses || [a.license]).filter(Boolean))].sort(), architectures: [...new Set(catalog.flatMap(a=>a.architectures || []))].sort(), snapshot_date:'2026-09-05', source_count:4 };
     }
     if (command === 'take_activation_errors') return [];
     if (command === 'managed_engine_status') return {bootstrap:{status:'not_configured'}, prerequisites:{state:'ready'}, daemon:'not_checked', disk_available_bytes:null};
     if (command === 'list_apps') return structuredClone(current);
     if (command === 'add_app') { current.push({ id:args.name.toLowerCase().replaceAll(' ','-'), display_name:args.name, launch_url:args.url, icon_path:null, runtime:{kind:'external'}, status:'connected', catalog_id:null, created_at_unix:1, updated_at_unix:1 }); return; }
     if (command === 'remove_app_cmd' || command === 'uninstall_app') { current = current.filter(a => a.id !== args.id); return; }
     if (command === 'recipe_details') return recipes[args.id];
     if (command === 'doctor') return { ready:true, checks:[{id:'docker',label:'Docker engine',ok:true,detail:'28.0.1'},{id:'compose',label:'Docker Compose',ok:true,detail:'2.35.1'}] };
     if (command === 'install_app') { const item=recipes[args.recipeId]; current.push({ id:item.id, display_name:item.display_name, launch_url:item.launch_url, icon_path:null, runtime:{kind:'compose'}, status:'running', catalog_id:item.display_name, created_at_unix:1, updated_at_unix:1 }); return; }
     if (command === 'app_logs') return 'memos  | server started on port 5230';
     if (command === 'start_app') { current.find(a=>a.id===args.id).status='running'; return; }
     if (command === 'stop_app') { current.find(a=>a.id===args.id).status='stopped'; return; }
   }}};
   window.__TAURI__.event = {listen: async (name, listener) => { if (name === 'local-store://activation-failure') { window.__activationFailureListener = listener; return () => { window.__activationFailureListener = null; }; } if (name === 'local-store://browser-failure') { window.__browserFailureListener = listener; return () => { window.__browserFailureListener = null; }; } window.__operationListener = listener; return () => { window.__operationListener = null; }; }};
 }, { catalog: fixtureCatalog, apps: fixtureApps, failure: options.failure, recipes: withSetup });
}
