import { useConnections, useUnlink, startLink } from '../../hooks/useConnections';

/** External accounts the user has linked through installed modules. */
export default function Connections() {
  const { data: providers = [], isLoading } = useConnections();
  const unlink = useUnlink();

  if (!isLoading && providers.length === 0) return null;

  return (
    <section className="space-y-4">
      <h2 className="text-sm font-medium text-[#71747a] uppercase tracking-wide">Connections</h2>
      <div className="space-y-3">
        {providers.map((p) => (
          <div
            key={`${p.module}:${p.id}`}
            className="p-4 rounded-lg bg-[#141517] border border-[#232529] flex items-center justify-between"
          >
            <div className="flex items-center gap-3 min-w-0">
              {p.icon_url && <img src={p.icon_url} alt="" className="w-8 h-8 rounded" />}
              <div className="min-w-0">
                <p className="text-sm font-medium text-[#e8eaed]">{p.name}</p>
                {p.linked && p.link?.display_name && (
                  <p className="text-xs text-[#71747a] truncate">
                    {p.link.profile_url ? (
                      <a href={p.link.profile_url} target="_blank" rel="noreferrer" className="hover:underline">
                        {p.link.display_name}
                      </a>
                    ) : (
                      p.link.display_name
                    )}
                  </p>
                )}
              </div>
            </div>
            {p.linked ? (
              <button
                onClick={() => unlink.mutate({ module: p.module, provider: p.id })}
                className="text-xs text-red-400 hover:text-red-300"
              >
                Disconnect
              </button>
            ) : (
              <button
                onClick={() => startLink(p.module, p.id)}
                className="text-xs px-3 py-1.5 rounded-md bg-[var(--accent)] text-black hover:bg-[var(--accent)]/90"
              >
                Connect
              </button>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
