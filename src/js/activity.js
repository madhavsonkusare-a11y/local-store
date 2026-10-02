import {escapeHtml, icon} from './render.js';
import {installStageLabel, operationLabel} from './operations.js';
const $ = id => document.getElementById(id);
const kinds = {install:'Install',start:'Start',stop:'Stop',uninstall:'Uninstall',open:'Open',shortcut:'Create shortcut',remove:'Remove connection'};
const states = {pending:'In progress',finishing:'Finishing',succeeded:'Completed',failed:'Failed',cancelled:'Stopped'};
const entries=[];
let sequence=0, statusFilter='';
// This history is memory only. Never store arguments, setup answers, app content,
// credentials or error messages; the owner's durable agent audit is separate.
function append(entry) {
  entries.unshift(entry);
  if(entries.length>100) entries.pop();
  paint(); return entry;
}
export function beginActivity(kind,appId,name) {
  const entry=append({id:`session-${++sequence}`,appId,name:name||appId,kind,state:'pending',startedAt:Date.now(),local:true,settled:false});
  return entry.id;
}
export function finishActivity(id,error) {
  const entry=entries.find(entry=>entry.id===id);
  if(!entry) return;
  entry.state=error ? error.code==='cancelled' ? 'cancelled' : 'failed' : 'succeeded';
  entry.errorCode=error && typeof error.code==='string' && /^[a-z_]{1,64}$/.test(error.code) ? error.code : error ? 'unknown' : null;
  entry.settled=true; entry.finishedAt=Date.now(); paint();
}
export function receiveActivity(event) {
  let entry=entries.find(entry=>entry.operationId===event.operation_id);
  if(!entry && event.state==='started') entry=entries.find(entry=>entry.local&&!entry.settled&&!entry.operationId&&entry.appId===event.app_id&&entry.kind===event.kind);
  if(!entry) entry=append({id:`session-${++sequence}`,appId:event.app_id.slice(0,120),name:event.app_id.slice(0,120),kind:event.kind,state:'pending',startedAt:Date.now(),local:false,settled:false});
  if(entry.settled) return;
  entry.operationId=event.operation_id.slice(0,120);
  if(event.state==='progress' && installStageLabel(event.stage)) entry.stage=event.stage;
  if(['succeeded','failed','cancelled'].includes(event.state)) {
    if(entry.local) entry.state='finishing'; // IPC settles launcher requests.
    else {entry.state=event.state;entry.settled=true;entry.finishedAt=Date.now();}
  }
  paint();
}
function paint() {
  const mount=$('activity-content');
  if(!mount) return;
  const expanded=new Set([...mount.querySelectorAll('details[open]')].map(detail=>detail.dataset.activityId));
  const focused=document.activeElement?.closest('[data-activity-id]')?.dataset.activityId;
  const query=$('activity-search').value.trim().toLowerCase();
  const matching=entries.filter(entry=>(!statusFilter||(statusFilter==='pending'?['pending','finishing'].includes(entry.state):entry.state===statusFilter))&&`${entry.name} ${entry.appId} ${kinds[entry.kind]}`.toLowerCase().includes(query));
  $('activity-count').textContent=`${matching.length} ${matching.length===1?'operation':'operations'} · This session`;
  mount.innerHTML=matching.length ? matching.map(entry=>{
    const time=new Date(entry.startedAt);
    const displayTime=time.toLocaleTimeString(undefined,{hour:'2-digit',minute:'2-digit'});
    const status=states[entry.state]||'Checking';
    const detail=entry.state==='pending' ? (installStageLabel(entry.stage)||operationLabel(entry.kind)) : entry.state==='finishing' ? 'Waiting for the request to finish.' : entry.state==='failed' ? 'Check the app status and logs before retrying.' : entry.state==='cancelled' ? 'Setup stopped. Review any retained setup if cleanup failed.' : 'The request completed.';
    return `<details class="activity-event" data-activity-id="${entry.id}"${expanded.has(entry.id)?' open':''}><summary><time datetime="${time.toISOString()}">${escapeHtml(displayTime)}</time><span class="activity-copy"><strong>${escapeHtml(kinds[entry.kind]||'Operation')} · ${escapeHtml(entry.name)}</strong><small>${escapeHtml(detail)}</small></span><span class="activity-state activity-state-${entry.state}">${escapeHtml(status)}</span></summary><dl class="activity-event-detail"><div><dt>App</dt><dd>${escapeHtml(entry.appId)}</dd></div><div><dt>Operation</dt><dd>${escapeHtml(entry.operationId||'Local request; backend event unavailable')}</dd></div>${entry.errorCode?`<div><dt>Failure code</dt><dd>${escapeHtml(entry.errorCode)}</dd></div>`:''}</dl></details>`;
  }).join('') : `<div class="activity-empty">${icon('layout-grid')}<h2>${entries.length?'No matching operations':'A quiet session so far.'}</h2><p>${entries.length?'Try another search or filter.':'Install, open or manage an app to see its activity here. History clears when Local Store closes.'}</p></div>`;
  if(focused) (mount.querySelector(`[data-activity-id="${focused}"] summary`) || $('activity-filters').querySelector('[aria-pressed="true"]'))?.focus({preventScroll:true});
}
$('activity-search')?.addEventListener('input',paint);
$('activity-filters')?.addEventListener('click',event=>{
  const button=event.target.closest('[data-activity-filter]'); if(!button) return;
  statusFilter=button.dataset.activityFilter;
  for(const control of $('activity-filters').querySelectorAll('button')) control.setAttribute('aria-pressed',String(control===button));
  paint();
});
export function renderActivity(){paint();}
