// --- APP STATE & MESSAGING LOGIC ---
let currentChat = null;
let lastMsgCount = 0;

const defaultDisplayName = '{{ user.split("@")[0] }}';
marked.setOptions({ gfm: true, breaks: true, headerIds: false, mangle: false });

// --- SETTINGS STATE LOGIC ---
let userSettings = JSON.parse(localStorage.getItem('atelierSettings')) || {
    displayName: defaultDisplayName,
    theme: 'dark',
    accentColor: '#7eafff'
};

function applySettings() {
    // Apply Variables
    document.documentElement.style.setProperty('--dynamic-primary', userSettings.accentColor);
    
    // Apply Theme Class
    if(userSettings.theme === 'light') {
        document.body.classList.add('theme-light');
    } else {
        document.body.classList.remove('theme-light');
    }

    // Apply Display Name to UI Elements
    document.getElementById('input-display-name').value = userSettings.displayName;
    document.getElementById('settings-display-name-preview').innerText = userSettings.displayName;
    
    // Initials Logic
    const initials = userSettings.displayName.substring(0, 2).toUpperCase();
    document.getElementById('sidebar-avatar').innerText = initials;
    document.getElementById('settings-avatar-preview').innerText = initials;
}


function updatePreview() {
    const val = document.getElementById('input-display-name').value || defaultDisplayName;
    document.getElementById('settings-display-name-preview').innerText = val;
    document.getElementById('settings-avatar-preview').innerText = val.substring(0, 2).toUpperCase();
}

function selectTheme(theme) {
    userSettings.theme = theme;
    // Visual feedback on cards can be added here
}

function selectAccent(hex) {
    userSettings.accentColor = hex;
    document.documentElement.style.setProperty('--dynamic-primary', hex); // Live preview
}

function saveSettings() {
    userSettings.displayName = document.getElementById('input-display-name').value || defaultDisplayName;
    localStorage.setItem('atelierSettings', JSON.stringify(userSettings));
    applySettings();
    
    // Visual feedback
    const btn = document.querySelector('footer button');
    const origText = btn.innerText;
    btn.innerText = "Saved!";
    setTimeout(() => btn.innerText = origText, 2000);
}

// --- VIEW CONTROLLER ---
function toggleView(viewName) {
    const chatView = document.getElementById('view-chat');
    const settingsView = document.getElementById('view-settings');
    const navChat = document.getElementById('nav-chat');
    const navSettings = document.getElementById('nav-settings');

    if (viewName === 'settings') {
        chatView.classList.add('opacity-0', 'pointer-events-none');
        settingsView.classList.remove('opacity-0', 'pointer-events-none');
        
        navChat.className = "text-[#f8f9fc]/50 p-3 hover:bg-[#1c2023] rounded-full transition-all";
        navSettings.className = "bg-[#3d4758] text-[#c7d1e6] rounded-full p-3 flex flex-col items-center transition-all duration-200";
    } else {
        settingsView.classList.add('opacity-0', 'pointer-events-none');
        chatView.classList.remove('opacity-0', 'pointer-events-none');
        
        navSettings.className = "text-[#f8f9fc]/50 p-3 hover:bg-[#1c2023] rounded-full transition-all";
        navChat.className = "bg-[#3d4758] text-[#c7d1e6] rounded-full p-3 mb-2 flex flex-col items-center transition-all duration-200";
        loadChats(); // refresh chats on return
    }
}

// --- EXISTING CHAT LOGIC (Preserved) ---
function uiStartNewChat() { /* ... original logic ... */
    currentChat = null; lastMsgCount = 0;
    document.getElementById('chat-header').innerText = "New Chat";
    document.getElementById('messages').innerHTML = '';
    const ncu = document.getElementById('new-chat-user');
    ncu.style.display = 'block'; ncu.value = ''; ncu.focus();
}

async function loadChats() {
    const res = await fetch('/api/chats');
    const chats = await res.json();
    const list = document.getElementById('chat-list');
    list.innerHTML = '';
    chats.forEach(chat => {
        const isActive = (currentChat === chat);
        const activeClasses = isActive ? 'bg-surface-container-highest border-l-4 border-primary' : 'hover:bg-surface-container-high';
        const div = document.createElement('div');
        div.className = `${activeClasses} rounded-lg p-4 flex items-center gap-4 cursor-pointer transition-all duration-200`;
        div.onclick = () => selectChat(chat);
        div.innerHTML = `
            <div class="relative flex-shrink-0">
                <div class="w-10 h-10 rounded-lg bg-primary/20 text-primary flex items-center justify-center font-bold">
                    ${chat.substring(0, 2).toUpperCase()}
                </div>
            </div>
            <div class="flex-1 min-w-0"><h3 class="font-bold text-sm truncate text-on-surface">${chat}</h3></div>
        `;
        list.appendChild(div);
    });
}

function selectChat(chat) {
    currentChat = chat; lastMsgCount = 0; 
    document.getElementById('chat-header').innerText = chat;
    document.getElementById('new-chat-user').style.display = 'none';
    loadMessages(); loadChats();
}

async function loadMessages() {
    if (!currentChat || typeof marked === 'undefined') return;
    const res = await fetch(`/api/messages/${encodeURIComponent(currentChat)}`);
    const msgs = await res.json();
    
    if (msgs.length !== lastMsgCount) {
        const container = document.getElementById('messages');
        container.innerHTML = '';
        msgs.forEach(m => {
            const isMe = m.sender === currentUser;
            let htmlContent = marked.parse(m.text);

            const temp = document.createElement('div');
            temp.innerHTML = htmlContent;
            temp.querySelectorAll('img').forEach(img => {
                const original = img.src;
                if(!original.includes(window.location.host + '/uploads/')) {
                    img.src = `/media/proxy?url=${encodeURIComponent(original)}`;
                }
                img.loading = "lazy";
            });
            
            htmlContent = temp.innerHTML;
            const div = document.createElement('div');
            
            // Using User's Custom Display Name for their own messages
            const displaySender = isMe ? userSettings.displayName : m.sender.split('@')[0];

            if (isMe) {
                div.className = "flex flex-row-reverse items-end gap-4 max-w-2xl ml-auto group";
                div.innerHTML = `
                    <div class="flex-shrink-0 mb-1"><div class="w-8 h-8 rounded-lg bg-primary text-on-primary flex items-center justify-center font-bold text-[10px]">${displaySender.substring(0,2).toUpperCase()}</div></div>
                    <div class="space-y-1 items-end flex flex-col"><div class="bg-primary text-on-primary p-4 rounded-2xl rounded-br-none shadow-lg text-sm">${htmlContent}</div></div>
                `;
            } else {
                div.className = "flex items-end gap-4 max-w-2xl group";
                div.innerHTML = `
                    <div class="flex-shrink-0 mb-1"><div class="w-8 h-8 rounded-lg bg-surface-container-highest flex items-center justify-center text-[10px] font-bold">${displaySender.substring(0,2).toUpperCase()}</div></div>
                    <div class="space-y-1"><div class="bg-surface-container-high p-4 rounded-2xl rounded-bl-none shadow-sm text-sm">${htmlContent}</div><span class="text-[10px] text-on-surface-variant pl-1">${displaySender}</span></div>
                `;
            }
            container.appendChild(div);
        });
        container.scrollTop = container.scrollHeight;
        lastMsgCount = msgs.length;
    }
}

async function sendMessage() {
    const input = document.getElementById('msg-input').value;
    document.getElementById('msg-input').value = '';
    const text = input.trim();
    const receiver = currentChat || document.getElementById('new-chat-user').value.trim();
    if (!text || !receiver) return;

    const res = await fetch('/api/sendmessage', {
        method: 'POST', headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({receiver, messageText: text})
    });
    if (res.ok) { 
        input.value = '';
        if (!currentChat){
            selectChat(receiver);
        }
        else {
            loadMessages(); 
        } 
    }
    else {
        document.getElementById('msg-input').value = input;
    }
}

async function handleFileUpload() {
    const fileInput = document.getElementById('file-input');
    if (fileInput.files.length === 0) return;

    const formData = new FormData();
    formData.append('file', fileInput.files[0]);

    const res = await fetch('/api/upload', { method: 'POST', body: formData });
    if (res.ok) {
        const data = await res.json();
        document.getElementById('msg-input').value += ` ${data.markdown} `;
    } else {
        alert("Upload failed.");
    }
    fileInput.value = '';
}

// Initialization
applySettings(); // Load settings immediately on boot
function initApp() {
    if (typeof marked !== 'undefined') {
        marked.setOptions({ gfm: true, breaks: true });
        setInterval(() => { if (document.getElementById('view-chat').classList.contains('opacity-0') === false) { loadChats(); if (currentChat) loadMessages(); } }, 1000);
        loadChats();
    } else { setTimeout(initApp, 100); }
}
