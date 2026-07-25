// A test stand-in for the generated `ridl.std` module — NOT a shipped artifact.
//
// The corpus entries reference `ridl.std` types without importing them (the
// standard package is implicitly in scope), and the TypeScript backend emits
// `import * as ridl_std from './ridl.std'` for them. Nothing generates that
// module today, so `tsc` needs one to resolve against. This file is the
// TypeScript analogue of the `PRELUDE` constant the rustc proofs prepend, and
// it is written by hand for the same reason: it stands in for a dependency, it
// does not stand in for anything under test.
//
// The branding follows the convention in `backends/typescript/src/tests.rs`, so
// a generated reference that resolves to a bare `string` or `number` by
// accident does not type-check.

export type Name = string & { readonly __ridl: 'ridl.std.Name' };
export type Label = string & { readonly __ridl: 'ridl.std.Label' };
export type Message = string & { readonly __ridl: 'ridl.std.Message' };
export type Version = string & { readonly __ridl: 'ridl.std.Version' };
export type Timestamp = bigint & { readonly __ridl: 'ridl.std.Timestamp' };
export type Duration = number & { readonly __ridl: 'ridl.std.Duration' };

export function initName(): Name {
  return '' as Name;
}
export function initLabel(): Label {
  return '' as Label;
}
export function initMessage(): Message {
  return '' as Message;
}
export function initVersion(): Version {
  return '' as Version;
}
export function initTimestamp(): Timestamp {
  return 0n as Timestamp;
}
export function initDuration(): Duration {
  return 0 as Duration;
}
