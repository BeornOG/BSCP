import { useCall } from '../../hooks/useCall';

export default function IncomingCallModal() {
  const { incoming, accept, reject } = useCall();
  if (!incoming) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-72 rounded-2xl bg-[#151517] border border-[#232529] p-6 text-center">
        <span className="material-symbols-outlined text-[36px] text-green-400 animate-pulse">call</span>
        <p className="mt-2 text-[#e8eaed] font-medium">Incoming call</p>
        <p className="text-[#71747a] text-sm truncate">{incoming.from}</p>
        <div className="mt-5 flex gap-3">
          <button
            type="button"
            onClick={reject}
            className="flex-1 py-2 rounded-lg bg-red-600 hover:bg-red-500 text-white text-sm font-medium"
          >
            Decline
          </button>
          <button
            type="button"
            onClick={accept}
            className="flex-1 py-2 rounded-lg bg-green-600 hover:bg-green-500 text-white text-sm font-medium"
          >
            Accept
          </button>
        </div>
      </div>
    </div>
  );
}
