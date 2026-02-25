using System;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace RT.Interop;

/// <summary>
/// C-compatible result envelope returned by every <c>rtflow_*</c> native
/// function.  Mirrors the Rust <c>RtflowResult</c> repr(C) struct.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RtflowResult
{
    /// <summary><c>true</c> on success; <c>false</c> on failure.</summary>
    [MarshalAs(UnmanagedType.I1)]
    public bool Ok;

    /// <summary>
    /// Pointer to a null-terminated UTF-8 JSON string on success;
    /// <see cref="IntPtr.Zero"/> on failure.
    /// </summary>
    public IntPtr Data;

    /// <summary>
    /// Pointer to a null-terminated UTF-8 error message on failure;
    /// <see cref="IntPtr.Zero"/> on success.
    /// </summary>
    public IntPtr Error;
}

/// <summary>
/// Managed wrapper returned by <see cref="RustInterop.MarshalResult"/>.
/// The native memory has already been freed when this object is returned.
/// </summary>
public sealed class RtflowManagedResult
{
    /// <summary><c>true</c> when the native call succeeded.</summary>
    public bool Ok { get; init; }

    /// <summary>Deserialized JSON payload on success; <c>null</c> on failure.</summary>
    public string? Data { get; init; }

    /// <summary>Error message on failure; <c>null</c> on success.</summary>
    public string? Error { get; init; }
}

/// <summary>
/// P/Invoke declarations for the <c>rt_ffi</c> native shared library.
///
/// Every function that returns an <see cref="IntPtr"/> returns a pointer to a
/// heap-allocated <see cref="RtflowResult"/> struct.  Callers must pass that
/// pointer to <see cref="rtflow_free"/> exactly once when they are done with
/// it.  <see cref="MarshalResult"/> handles marshalling and freeing in a
/// single, safe call.
/// </summary>
public static class RustInterop
{
    private const string LibName = "rt_ffi";

    // -----------------------------------------------------------------------
    // Memory management
    // -----------------------------------------------------------------------

    /// <summary>
    /// Free a <c>RtflowResult</c> (and its inner strings) allocated by any
    /// <c>rtflow_*</c> function.  Passing <see cref="IntPtr.Zero"/> is a
    /// no-op.
    /// </summary>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void rtflow_free(IntPtr ptr);

    // -----------------------------------------------------------------------
    // Database
    // -----------------------------------------------------------------------

    /// <summary>
    /// Initialize or open the SQLite database at <paramref name="dbPath"/>.
    /// </summary>
    /// <param name="dbPath">Filesystem path to the SQLite file.</param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c>.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_init(string dbPath);

    // -----------------------------------------------------------------------
    // Document ingestion
    // -----------------------------------------------------------------------

    /// <summary>
    /// Ingest a JSON array of blocks into the store under the given document
    /// UUID.
    /// </summary>
    /// <param name="json">Serialized block array.</param>
    /// <param name="docId">UUID string identifying the document.</param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c>.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_ingest_blocks(string json, string docId);

    // -----------------------------------------------------------------------
    // Compare
    // -----------------------------------------------------------------------

    /// <summary>
    /// Compare two documents and return a <c>CompareResult</c> JSON object.
    /// </summary>
    /// <param name="leftDocId">UUID of the left (base) document.</param>
    /// <param name="rightDocId">UUID of the right (incoming) document.</param>
    /// <param name="optionsJson">
    /// JSON object with compare options.  Pass <c>"{}"</c> for defaults.
    /// </param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c>.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_compare(
        string leftDocId,
        string rightDocId,
        string optionsJson);

    // -----------------------------------------------------------------------
    // Merge
    // -----------------------------------------------------------------------

    /// <summary>
    /// Merge an incoming document into a base document and return a
    /// <c>MergeResult</c> JSON object.
    /// </summary>
    /// <param name="baseDocId">UUID of the base document.</param>
    /// <param name="incomingDocId">UUID of the incoming document.</param>
    /// <param name="optionsJson">
    /// JSON object with merge options.  Pass <c>"{}"</c> for defaults.
    /// </param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c>.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_merge(
        string baseDocId,
        string incomingDocId,
        string optionsJson);

    // -----------------------------------------------------------------------
    // Workflow
    // -----------------------------------------------------------------------

    /// <summary>
    /// Submit a workflow event and advance the workflow state machine.
    /// </summary>
    /// <param name="workflowId">UUID of the workflow.</param>
    /// <param name="eventJson">JSON object describing the event.</param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c> containing the updated
    /// <c>WorkflowState</c> JSON on success.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_workflow_event(
        string workflowId,
        string eventJson);

    /// <summary>
    /// Retrieve the current state of a workflow.
    /// </summary>
    /// <param name="workflowId">UUID of the workflow.</param>
    /// <returns>
    /// Pointer to a <c>RtflowResult</c> containing the current
    /// <c>WorkflowState</c> JSON on success.  Must be freed with
    /// <see cref="rtflow_free"/>.
    /// </returns>
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr rtflow_workflow_state(string workflowId);

    // -----------------------------------------------------------------------
    // Marshalling helper
    // -----------------------------------------------------------------------

    /// <summary>
    /// Marshal the <see cref="RtflowResult"/> pointed to by <paramref name="ptr"/>
    /// into a managed <see cref="RtflowManagedResult"/>, then free the native
    /// memory via <see cref="rtflow_free"/>.
    /// </summary>
    /// <param name="ptr">
    /// Non-null pointer returned by any <c>rtflow_*</c> function.
    /// </param>
    /// <returns>A managed copy of the result with no live native references.</returns>
    /// <exception cref="ArgumentNullException">
    /// Thrown when <paramref name="ptr"/> is <see cref="IntPtr.Zero"/>.
    /// </exception>
    public static RtflowManagedResult MarshalResult(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
            throw new ArgumentNullException(nameof(ptr),
                "rtflow_* returned a null pointer; this indicates a fatal allocator failure.");

        try
        {
            // Marshal the struct out of native memory.
            var raw = Marshal.PtrToStructure<RtflowResult>(ptr);

            string? data  = raw.Data  != IntPtr.Zero ? Marshal.PtrToStringUTF8(raw.Data)  : null;
            string? error = raw.Error != IntPtr.Zero ? Marshal.PtrToStringUTF8(raw.Error) : null;

            return new RtflowManagedResult
            {
                Ok    = raw.Ok,
                Data  = data,
                Error = error,
            };
        }
        finally
        {
            // Always free the native envelope, even if marshalling threw.
            rtflow_free(ptr);
        }
    }
}
