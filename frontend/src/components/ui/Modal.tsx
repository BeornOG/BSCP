import { useEffect, type FC, type ReactNode } from 'react';

interface ModalProps {
  open?: boolean;
  onClose: () => void;
  children: ReactNode;
  title?: string;
}

const Modal: FC<ModalProps> = ({ open, onClose, children, title }) => {
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm transition-opacity duration-200"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-lg mx-4 bg-[#141517] border border-[#232529] rounded-2xl shadow-2xl animate-in fade-in zoom-in-95 duration-200"
        onClick={(e) => e.stopPropagation()}
      >
        {title && (
          <div className="flex items-center justify-between px-6 py-4 border-b border-[#232529]">
            <h2 className="text-base font-semibold text-[#e8eaed]">{title}</h2>
            <button
              onClick={onClose}
              className="text-[#71747a] hover:text-[#e8eaed] transition-colors"
            >
              <span className="material-symbols-outlined text-xl">close</span>
            </button>
          </div>
        )}
        {!title && (
          <button
            onClick={onClose}
            className="absolute top-4 right-4 text-[#71747a] hover:text-[#e8eaed] transition-colors"
          >
            <span className="material-symbols-outlined text-xl">close</span>
          </button>
        )}
        <div className="px-6 py-4">{children}</div>
      </div>
    </div>
  );
};

export default Modal;
