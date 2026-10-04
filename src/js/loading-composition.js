// Skeletons reuse the approved primitive and contain no invented app facts.
export function loadingComposition(label, kind = 'summary') {
  const shapes = kind === 'logs' ? '<div class="logs-loading"><div class="log-skeleton"></div><div class="log-skeleton short"></div><div class="log-skeleton"></div><div class="log-skeleton medium"></div></div>' : kind === 'apps'
    ? `<div class="my-apps-layout"><div class="installed-list">${Array.from({length:5},()=>'<div class="app-row-skeleton"><i></i><span></span></div>').join('')}</div><div class="my-apps-detail"><div class="detail-head-skeleton"></div><div class="detail-tabs-skeleton"></div><div class="detail-body-skeleton"></div></div></div>`
    : kind === 'install'
      ? '<div class="install-loading-layout"><div class="skeleton hero"></div><div class="skeleton install-review-skeleton"></div></div>'
      : '<div class="skeleton-stack"><div class="skeleton hero"></div><div class="skeleton-row"><div class="skeleton"></div><div class="skeleton"></div><div class="skeleton"></div></div><div class="skeleton loading-body"></div></div>';
  return `<div class="production-loading" role="status" aria-busy="true"><p>${label}</p><div aria-hidden="true">${shapes}</div></div>`;
}
