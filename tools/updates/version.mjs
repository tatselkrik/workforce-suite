// Bounded SemVer spelling for locally signed artifacts. The hosted stable feed
// retains its numeric-only contract; beta delivery uses the private Git release.
export function validReleaseVersion(value) {
  if (typeof value !== 'string' || value.length > 80) return false;
  const match = /^(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(value);
  return Boolean(match && match[0] === value && (!match[4] || match[4].split('.').every(id => !/^[0-9]+$/.test(id) || id === '0' || !id.startsWith('0'))));
}
