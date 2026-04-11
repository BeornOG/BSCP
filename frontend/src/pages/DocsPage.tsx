export default function DocsPage() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between px-6 py-4 border-b border-[#232529]">
        <h1 className="text-lg font-semibold text-[#e8eaed]">API Documentation</h1>
        <a
          href="/api/docs/"
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm text-[var(--accent)] hover:underline"
        >
          Open in new tab
        </a>
      </div>
      <iframe
        src="/api/docs/"
        title="API Documentation"
        className="flex-1 w-full border-0"
        style={{ colorScheme: 'light' }}
      />
    </div>
  );
}
