// P/Invoke surface of crates/sim-ffi (libsim_ffi.dylib / sim_ffi.dll in Assets/Plugins).
// Mirrors crates/sim-ffi/src/lib.rs one-to-one — see docs/sim-ffi-unity-gap.md.
//
// RULES (each one is a silent-corruption bug if broken):
//   - Every char* the library RETURNS is Rust-owned: receive it as IntPtr, copy it with
//     TakeString(), which frees it with roi_string_free. Never declare those returns as `string`
//     (the .NET marshaller would free a Rust allocation with the wrong allocator).
//   - Strings passed IN are marshalled as UTF-8 for the duration of the call only.
//   - One thread (the main thread) per handle. Ship the release dylib only (panic = abort).
using System;
using System.Runtime.InteropServices;

namespace Roi.Sim
{
    public static class SimFfi
    {
#if UNITY_IOS && !UNITY_EDITOR
        const string Lib = "__Internal";
#else
        const string Lib = "sim_ffi";
#endif
        const CallingConvention CC = CallingConvention.Cdecl;

        // ---- lifecycle -------------------------------------------------------------------------
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_new(ulong seed,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string zoneId);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_new_as(ulong seed,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string zoneId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string unitDef,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string partyName);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_new_starter(ulong seed,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string zoneId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string partyName);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_load_json(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string saveJson,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_clone(IntPtr sim);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern void roi_free(IntPtr sim);

        // ---- driving ---------------------------------------------------------------------------
        [DllImport(Lib, CallingConvention = CC)]
        public static extern void roi_tick(IntPtr sim, ulong n);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern int roi_apply_input(IntPtr sim,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string eventJson);

        // ---- reading (returns are Rust-owned; use TakeString) ------------------------------------
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_view_json(IntPtr sim);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_save_json(IntPtr sim);

        // One named collection — inventory / stash / escrow / rare_bag / card_album /
        // exemplar_units / owned_regalia. These are the lists the per-tick view empties (the bag
        // alone stutters at 10 Hz), so a screen asks for the one it needs when it opens.
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_query_json(IntPtr sim,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string key);

        // DEV CHEATS: one entry point taking a command, mirroring roi_query_json — a C ABI pays for
        // every symbol twice, and the Godot bridge grew a dozen separate debug_* functions this
        // deliberately does not repeat. These are DIRECT state edits and NOT inputs, so a cheated
        // save stops verifying rather than replaying as legitimate play. Returns 1 if it applied.
        [DllImport(Lib, CallingConvention = CC)]
        public static extern int roi_debug_json(IntPtr sim,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string cmd);

        // Refuse the cheats. The driver MUST call this with 0 at login: a direct edit is absent
        // from the input log, so every push after one would be rejected.
        [DllImport(Lib, CallingConvention = CC)]
        public static extern void roi_set_debug_allowed(IntPtr sim, int allowed);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_state_hash(IntPtr sim);

        // ---- online ----------------------------------------------------------------------------
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_export_log(IntPtr sim);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern void roi_commit_anchor(IntPtr sim);

        [DllImport(Lib, CallingConvention = CC)]
        public static extern long roi_replay_span(IntPtr sim);

        // ---- identity --------------------------------------------------------------------------
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_sim_version();
        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_sim_build();

        [DllImport(Lib, CallingConvention = CC)]
        public static extern IntPtr roi_sim_git_stamp();

        [DllImport(Lib, CallingConvention = CC)]
        public static extern void roi_string_free(IntPtr p);

        /// Copy a Rust-owned string and free it. Null pointer -> null.
        public static string TakeString(IntPtr p)
        {
            if (p == IntPtr.Zero) return null;
            try { return Marshal.PtrToStringUTF8(p); }
            finally { roi_string_free(p); }
        }
    }
}
