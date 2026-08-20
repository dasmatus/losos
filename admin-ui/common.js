'use strict';

/* losos admin UI — shared helpers used by both the dashboard (/) and the
 * settings page (/settings/): DOM lookup, admin token storage in
 * sessionStorage ('losos-token'), Bearer auth headers, and the Tahoe WUI
 * URL (cross-origin on :3456). Loaded via a plain <script> tag before each
 * page's own app.js — no modules, just top-level functions like the rest
 * of this dependency-free SPA. */

function $(id) { return document.getElementById(id); }

function getToken() { return sessionStorage.getItem('losos-token'); }
function saveToken(t) { sessionStorage.setItem('losos-token', t); }
function dropToken() { sessionStorage.removeItem('losos-token'); }

function authHeaders() {
  const t = getToken();
  return t ? { 'Authorization': 'Bearer ' + t } : {};
}

function tahoeUrl() {
  return location.protocol + '//' + location.hostname + ':3456/';
}
