import {installStageLabel} from './operations.js';
const progress=document.getElementById('install-progress');
const stages=['checking_system','preparing_files','validating_recipe','starting_containers','waiting_for_health','saving_app'];
const list=document.createElement('ol');list.className='install-stage-list';list.setAttribute('aria-label','Installation stages');list.setAttribute('aria-live','off');progress.querySelector('p').after(list);
const outcome=document.createElement('section');outcome.id='install-outcome';outcome.className='install-outcome';outcome.hidden=true;outcome.setAttribute('aria-labelledby','install-outcome-title');
outcome.innerHTML='<p class="eyebrow" id="install-outcome-state"></p><h3 id="install-outcome-title"></h3><p id="install-outcome-copy"></p><p id="install-outcome-code" class="field-hint"></p><button id="install-outcome-recovery" class="secondary" hidden>Review retained setup</button>';
document.getElementById('install-error').before(outcome);
export function resetInstallTask(){outcome.hidden=true;renderInstallStage(null);}
export function renderInstallStage(stage){
  if(!stage){list.hidden=true;return;}
  list.hidden=false;const current=stages.indexOf(stage);list.replaceChildren();
  for(const[key,index]of stages.map((key,index)=>[key,index])){const item=document.createElement('li');const name=document.createElement('span');name.textContent=installStageLabel(key).replace(/…$/,'');const state=document.createElement('small');state.textContent=index===current?'Current':index<current?'Earlier':'Upcoming';if(index===current)item.setAttribute('aria-current','step');item.append(name,state);list.append(item);}
  if(stage==='rolling_back'){const item=document.createElement('li');item.textContent='Cleaning up the incomplete setup';item.setAttribute('aria-current','step');list.append(item);}
}
export function renderInstallFailure(error){
  const unsafe=error?.code==='rollback_failed',cancelled=error?.code==='cancelled';
  outcome.hidden=false;outcome.dataset.state=unsafe?'unsafe':cancelled?'cancelled':'failed';
  document.getElementById('install-outcome-state').textContent=unsafe?'REVIEW REQUIRED':cancelled?'SETUP STOPPED':'SETUP DID NOT FINISH';
  document.getElementById('install-outcome-title').textContent=unsafe?'Check what was left behind':cancelled?'Nothing was added to My Apps':'Review the issue before trying again';
  document.getElementById('install-outcome-copy').textContent=unsafe?'Cleanup could not finish. Review the retained setup before retrying; data may still be on this computer.':cancelled?'The setup request was canceled. Existing retained data follows the app’s keep-data policy.':'The installation did not complete. Your review and entered answers remain available below.';
  document.getElementById('install-outcome-code').textContent=error?.code?`Result code: ${error.code}`:'';
  document.getElementById('install-outcome-recovery').hidden=!unsafe;
}
