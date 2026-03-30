// --- APP STATE & MESSAGING LOGIC ---
let currentChat = null;
let chatSessionId = 0; // NEW: Tracks the active chat to prevent data bleed
let loadedMessageIds = new Set();
let oldestMsgTime = null;
let newestMsgTime = null;
let isFetchingMessages = false;

// Declared exactly once, right at the top
//const defaultDisplayName = '{{ user.split("@")[0] }}';
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

    // Update theme card visual feedback
    document.querySelectorAll('.theme-card').forEach(card => {
        const cardTheme = card.onclick.toString().includes("'light'") ? 'light' : 'dark';
        if (cardTheme === userSettings.theme) {
            card.classList.add('border-primary', 'border-2');
            card.classList.remove('border-transparent');
        } else {
            card.classList.remove('border-primary');
            card.classList.add('border-transparent');
        }
    });
}


function updatePreview() {
    const val = document.getElementById('input-display-name').value || defaultDisplayName;
    document.getElementById('settings-display-name-preview').innerText = val;
    document.getElementById('settings-avatar-preview').innerText = val.substring(0, 2).toUpperCase();
}

function selectTheme(theme) {
    userSettings.theme = theme;
    applySettings(); // Apply immediately for live preview

    // Update visual feedback on theme cards
    document.querySelectorAll('.theme-card').forEach(card => {
        const cardTheme = card.onclick.toString().includes("'light'") ? 'light' : 'dark';
        if (cardTheme === theme) {
            card.classList.add('border-primary', 'border-2');
            card.classList.remove('border-transparent');
        } else {
            card.classList.remove('border-primary');
            card.classList.add('border-transparent');
        }
    });
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
function uiStartNewChat() {
    currentChat = null; 
    loadedMessageIds.clear();
    oldestMsgTime = null;
    newestMsgTime = null;
    document.getElementById('chat-header').innerText = "New Chat";
    document.getElementById('messages').innerHTML = '';
    const ncu = document.getElementById('new-chat-user');
    ncu.style.display = 'block'; ncu.value = ''; ncu.focus();
}

async function loadChats() {
    try {
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
    } catch (err) {
        console.error("Failed to load chats:", err);
    }
}

function selectChat(chat) {
    currentChat = chat; 
    chatSessionId++; // Invalidate any pending network requests from the previous chat
    isFetchingMessages = false; // Force unlock so the new chat loads instantly
    loadedMessageIds.clear();
    oldestMsgTime = null;
    newestMsgTime = null;
    document.getElementById('chat-header').innerText = chat;
    document.getElementById('new-chat-user').style.display = 'none';
    document.getElementById('messages').innerHTML = ''; // Clear container
    loadMessages(); 
    loadChats();
}

function convertPlainMediaUrls(text) {
    const imageExtensions = /\.(png|jpg|jpeg|gif|webp|bmp|svg)(\?.*)?$/i;
    const videoExtensions = /\.(mp4|webm|mov|avi|mkv|flv|wmv|m4v)(\?.*)?$/i;

    // Check if entire text is already markdown format
    if (/^!\[.*\]\(https?:\/\/[^\)]+\)$/.test(text) || /^<video[\s\S]*<\/video>$/.test(text)) {
        return text; // Already markdown or HTML, skip processing
    }

    const urlRegex = /(https?:\/\/[^\s<>"'\)]+)/g;
    return text.replace(urlRegex, (match) => {
        if (imageExtensions.test(match)) {
            return `![image](${match})`;
        } else if (videoExtensions.test(match)) {
            return `<video controls style="max-width: 100%; border-radius: 8px; margin: 8px 0;"><source src="${match}"></video>`;
        }
        return match;
    });
}

function createMessageElement(m) {
    // Handle sender format (could be "user" or "user@domain")
    const mSenderUsername = m.sender ? m.sender.split('@')[0] : "Unknown";
    const currentUsername = currentUser.split('@')[0];
    const isMe = mSenderUsername === currentUsername;

    // Safety Fallbacks: Prevents fatal errors if the database has a null text/sender row
    const safeText = m.text || "";
    const safeSender = mSenderUsername;

    const textWithMediaMarkdown = convertPlainMediaUrls(safeText);
    let htmlContent = marked.parse(textWithMediaMarkdown);

    // Proxy images logic
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
    
    // Parse Unix Timestamp (float seconds to milliseconds)
    const date = new Date((m.time || 0) * 1000);
    const timeString = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

    const div = document.createElement('div');
    const displaySender = isMe ? userSettings.displayName : safeSender;

    if (isMe) {
        div.className = "flex flex-row-reverse items-end gap-4 max-w-2xl ml-auto group mt-2";
        div.innerHTML = `
            <div class="flex-shrink-0 mb-1"><div class="w-8 h-8 rounded-lg bg-primary text-on-primary flex items-center justify-center font-bold text-[10px]">${displaySender.substring(0,2).toUpperCase()}</div></div>
            <div class="space-y-1 items-end flex flex-col">
                <div class="bg-primary text-on-primary p-4 rounded-2xl rounded-br-none shadow-lg text-sm">${htmlContent}</div>
                <span class="text-[10px] text-on-surface-variant pr-1">${timeString}</span>
            </div>
        `;
    } else {
        div.className = "flex items-end gap-4 max-w-2xl group mt-2";
        div.innerHTML = `
            <div class="flex-shrink-0 mb-1"><div class="w-8 h-8 rounded-lg bg-surface-container-highest flex items-center justify-center text-[10px] font-bold">${displaySender.substring(0,2).toUpperCase()}</div></div>
            <div class="space-y-1 flex flex-col items-start">
                <div class="bg-surface-container-high p-4 rounded-2xl rounded-bl-none shadow-sm text-sm">${htmlContent}</div>
                <span class="text-[10px] text-on-surface-variant pl-1">${displaySender} &bull; ${timeString}</span>
            </div>
        `;
    }
    return div;
}

// fetchOlder decides if we are scrolling UP for history, or polling DOWN for new messages
async function loadMessages(fetchOlder = false) {
    if (!currentChat || typeof marked === 'undefined' || isFetchingMessages) return;
    isFetchingMessages = true;

    const mySession = chatSessionId; // Capture the session at the start of the fetch
    const requestedChat = currentChat;

    try {
        let url = `/api/messages/${encodeURIComponent(requestedChat)}`;
        
        const params = new URLSearchParams();
        if (fetchOlder && oldestMsgTime) params.append('before', oldestMsgTime);
        if (!fetchOlder && newestMsgTime) params.append('after', newestMsgTime);
        
        const qs = params.toString();
        if (qs) url += `?${qs}`;

        const res = await fetch(url);
        
        // RACE CONDITION CHECK: Did the user switch chats while waiting for the network?
        if (chatSessionId !== mySession) return; 

        if (!res.ok) throw new Error(`Server returned HTTP ${res.status}`);
        
        let data = await res.json();
        
        // DOUBLE CHECK: Just in case JSON parsing was delayed
        if (chatSessionId !== mySession) return;

        let msgs = Array.isArray(data) ? data : (data.messages || data.data || []);
        if (!Array.isArray(msgs) || msgs.length === 0) return;

        msgs.sort((a, b) => a.time - b.time);

        const container = document.getElementById('messages');
        const prevScrollHeight = container.scrollHeight;
        const prevScrollTop = container.scrollTop;
        const isNearBottom = container.scrollHeight - container.scrollTop <= container.clientHeight + 100;

        let addedAny = false;
        let elementsToAdd = [];

        msgs.forEach(m => {
            const msgId = m.id || `${m.time}-${m.text}`;
            if (loadedMessageIds.has(msgId)) return;
            
            loadedMessageIds.add(msgId);
            addedAny = true;

            if (!oldestMsgTime || m.time < oldestMsgTime) oldestMsgTime = m.time;
            if (!newestMsgTime || m.time > newestMsgTime) newestMsgTime = m.time;

            elementsToAdd.push(createMessageElement(m));
        });

        if (addedAny) {
            if (fetchOlder) {
                elementsToAdd.reverse().forEach(el => container.insertBefore(el, container.firstChild));
                container.offsetHeight; // Force layout recalculation
                container.scrollTop = prevScrollTop + (container.scrollHeight - prevScrollHeight);
            } else {
                elementsToAdd.forEach(el => container.appendChild(el));
                if (isNearBottom || prevScrollHeight === 0) {
                    container.scrollTop = container.scrollHeight;
                }
            }
        }
    } catch (err) {
        // Only log errors if we are still in the chat that caused them
        if (chatSessionId === mySession) console.error("Error loading messages:", err);
    } finally {
        // Only release the lock if we are still in the same chat
        if (chatSessionId === mySession) isFetchingMessages = false;
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
document.addEventListener('DOMContentLoaded', applySettings);
function initApp() {
    if (typeof marked !== 'undefined') {
        marked.setOptions({ gfm: true, breaks: true });
        
        // Add scroll listener for Infinite Pagination (Scroll Up)
        const msgContainer = document.getElementById('messages');
        if(msgContainer) {
            msgContainer.addEventListener('scroll', () => {
                // If user scrolls within 50px of the top, fetch older messages
                if (msgContainer.scrollTop <= 50) {
                    loadMessages(true);
                }
            });
        }

        // Standard polling for NEW messages
        setInterval(() => { 
            if (document.getElementById('view-chat').classList.contains('opacity-0') === false) { 
                loadChats(); 
                if (currentChat) loadMessages(false); 
            } 
        }, 1000);
        
        loadChats();
    } else { 
        setTimeout(initApp, 100); 
    }
}
