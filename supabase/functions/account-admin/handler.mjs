// Dependency-free handler shared with local tests. Never log request bodies, keys or tokens.
const roles = new Set(['administrator', 'hr_generalist', 'manager', 'employee']);
const uuid = value => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
const email = value => typeof value === 'string' && value.length <= 254 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
const answer = (status, message, extra = {}) => Response.json({ message, ...extra }, {
  status, headers: { 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' },
});

async function boundedBody(request) {
  const reader = request.body?.getReader();
  if (!reader) throw new Error('Missing body');
  const chunks = [];
  let length = 0;
  while (true) {
    const {value, done} = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > 8192) { await reader.cancel(); throw new RangeError('Body limit'); }
    chunks.push(value);
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {bytes.set(chunk, offset); offset += chunk.byteLength;}
  return JSON.parse(new TextDecoder('utf-8', {fatal:true}).decode(bytes));
}

export function createAccountHandler({ url, serviceKey, anonKey }, fetcher = fetch) {
  return async request => {
    if (request.method !== 'POST') return answer(405, 'Use POST.');
    if (request.headers.has('Origin')) return answer(403, 'This endpoint accepts authenticated desktop requests.');
    if (!url || !serviceKey || !anonKey || !url.startsWith('https://')) return answer(503, 'Account provisioning is unavailable. Contact your administrator.');
    const authorization = request.headers.get('Authorization') || '';
    if (authorization.length > 8192 || !/^Bearer [^\s]+$/.test(authorization)) return answer(401, 'Sign in before managing accounts.');
    if (!(request.headers.get('Content-Type') || '').startsWith('application/json')) return answer(415, 'Use JSON.');
    let body;
    try {
      body = await boundedBody(request);
      if (!body || typeof body !== 'object' || Array.isArray(body)) throw new Error();
    } catch (error) {
      return error instanceof RangeError ? answer(413, 'Account request is too large.') : answer(400, 'Enter valid account details.');
    }
    const base = url.replace(/\/$/, '');
    const call = (path, method, payload, admin = false) => fetcher(base + path, {
      method, redirect: 'error', signal: AbortSignal.timeout(12000),
      headers: { apikey: admin ? serviceKey : anonKey,
        Authorization: admin ? 'Bearer ' + serviceKey : authorization, 'Content-Type': 'application/json' },
      body: payload === undefined ? undefined : JSON.stringify(payload),
    });
    let createdUser;
    let assigned = false;
    try {
      // Validate the bearer on the Auth server; never trust unverified decoded claims.
      const identityResponse = await call('/auth/v1/user', 'GET');
      if (!identityResponse.ok) return answer(401, 'Your session could not be verified. Sign in again.');
      const identity = await identityResponse.json();
      if (!uuid(identity.id)) return answer(401, 'Your session could not be verified.');
      const accessResponse = await call('/rest/v1/rpc/my_hris_access', 'POST', {});
      if (!accessResponse.ok) return answer(403, 'Only active administrators can manage accounts.');
      const access = await accessResponse.json();
      if (access.user_id !== identity.id || access.role !== 'administrator') return answer(403, 'Only active administrators can manage accounts.');

      if (body.operation === 'resend') {
        if (!uuid(body.user_id)) return answer(400, 'Choose an account.');
        const listResponse = await call('/rest/v1/rpc/list_company_accounts', 'POST', {});
        if (!listResponse.ok) return answer(403, 'Refresh your administrator access.');
        const account = (await listResponse.json()).find(a => a.user_id === body.user_id && a.active);
        if (!account || account.email_confirmed) return answer(400, 'Choose an active account awaiting email confirmation.');
        const sent = await call('/auth/v1/resend', 'POST', { type: 'signup', email: account.email });
        if (!sent.ok) return answer(sent.status === 429 ? 429 : 502, 'The confirmation email could not be sent. Wait and try again.');
        return answer(200, 'Confirmation email sent.', { confirmation_sent: true });
      }
      if (body.operation !== 'create') return answer(400, 'Choose a supported account operation.');
      const targetEmail = typeof body.email === 'string' ? body.email.trim() : '';
      if (!email(targetEmail) || !roles.has(body.role) || (body.active !== undefined && typeof body.active !== 'boolean') || typeof body.password !== 'string' ||
          [...body.password].length < 12 || [...body.password].length > 128 ||
          (body.employee_id != null && !uuid(body.employee_id)) ||
          (body.manager_user_id != null && !uuid(body.manager_user_id)) ||
          (body.active !== false && ['manager', 'employee'].includes(body.role) && !body.employee_id)) {
        return answer(400, 'Enter a valid email, role, linked employee and a 12–128 character initial password.');
      }
      // Provisioning uses the server credential only here, after checking the live administrator membership.
      // Email ownership is not auto-confirmed. The client never receives this credential or a user's token.
      const created = await call('/auth/v1/admin/users', 'POST', {
        email: targetEmail, password: body.password, email_confirm: false,
      }, true);
      if (!created.ok) {
        const failure = await created.json().catch(() => ({}));
        if (['email_exists', 'user_already_exists'].includes(failure.error_code) || created.status === 409) {
          return answer(409, 'An account already exists for this email. Choose Existing account.');
        }
        return answer(400, 'The account could not be created. Check the email and password policy.');
      }
      const user = await created.json();
      if (!uuid(user.id)) return answer(502, 'The account service returned an unexpected response.');
      createdUser = user.id;
      // The caller's JWT, not the server key, authorizes the assignment. The RPC rechecks access under its lock.
      const saved = await call('/rest/v1/rpc/save_company_account', 'POST', {
        p_user_id: user.id, p_email: targetEmail, p_role: body.role, p_active: body.active !== false,
        p_employee_id: body.employee_id || null, p_manager_user_id: body.manager_user_id || null,
      });
      if (!saved.ok) return answer(409, 'Account access could not be assigned. Refresh and check the employee and reporting manager.');
      assigned = true;
      const sent = await call('/auth/v1/resend', 'POST', { type: 'signup', email: targetEmail });
      return answer(201, sent.ok ? 'Account created. Confirm the email before signing in.' :
        'Account created. Use Send confirmation to retry email delivery.', { confirmation_sent: sent.ok });
    } catch {
      // A delivery failure after the membership was committed must not delete the new account.
      if (assigned) return answer(201, 'Account created. Use Send confirmation to retry email delivery.', { confirmation_sent: false });
      return answer(502, 'Account provisioning could not finish. Refresh before trying again.');
    } finally {
      // Roll back only the exact new Auth user when assignment did not commit. Never delete an existing account.
      if (createdUser && !assigned) {
        try { await call('/auth/v1/admin/users/' + createdUser, 'DELETE', undefined, true); } catch { /* Orphan has no HRIS access. */ }
      }
    }
  };
}
