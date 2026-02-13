import { Component, OnInit, HostListener, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, KafkaConfig, MessageEntry, ConsumedMessage } from './services/tauri.service';

/** Check if running inside the Tauri webview */
function isTauri(): boolean {
  return !!(window as any).__TAURI_INTERNALS__;
}

type UnlistenFn = () => void;

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css',
})
export class AppComponent implements OnInit, OnDestroy {
  // Configuration
  config: KafkaConfig = {
    broker: 'localhost:9092',
    topic: 'test-topic',
    client_id: 'kafka-msg-publisher',
    security_protocol: 'Plaintext',
    sasl_mechanism: 'Plain',
    sasl_username: '',
    sasl_password: '',
    ssl_ca_cert_path: '',
    ssl_client_cert_path: '',
    ssl_client_key_path: '',
    ssl_skip_verification: false
  };

  // Message input
  messageContent = '';
  
  // Message history
  messages: MessageEntry[] = [];
  
  // UI state
  isLoading = false;
  isSending = false;
  showSettings = false;
  isDragOver = false;
  isTesting = false;
  testingCancelled = false;
  connectionStatus: 'unknown' | 'connected' | 'error' | 'testing' = 'unknown';
  
  // Theme
  isDarkMode = true;
  showSecuritySettings = false;

  // Create Topic
  showCreateTopic = false;
  newTopicName = '';
  newTopicPartitions = 1;
  newTopicReplication = 1;
  isCreatingTopic = false;
  topicCreateStatus: 'none' | 'success' | 'error' = 'none';
  topicCreateMessage = '';

  // Consumer / Message Viewer
  showConsumer = false;
  consumeTopic = '';
  consumeOffset = 0;
  consumeMaxMessages = 50;
  isConsuming = false;
  consumedMessages: ConsumedMessage[] = [];
  consumeError = '';

  // Message Detail Modal
  selectedMessage: ConsumedMessage | null = null;
  copiedField: string | null = null;

  // Tauri event listener
  private unlistenFileDrop: UnlistenFn | null = null;

  constructor(private tauriService: TauriService) {}

  async ngOnInit() {
    await this.loadConfig();
    await this.setupFileDropListener();
    this.loadThemePreference();
    
    // Auto-test connection on startup with spinner
    this.checkConnectionWithSpinner();
  }

  ngOnDestroy() {
    if (this.unlistenFileDrop) {
      this.unlistenFileDrop();
    }
  }

  loadThemePreference() {
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme === 'light') {
      this.isDarkMode = false;
      document.documentElement.setAttribute('data-theme', 'light');
    }
  }

  toggleTheme() {
    this.isDarkMode = !this.isDarkMode;
    const theme = this.isDarkMode ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }

  async setupFileDropListener() {
    if (!isTauri()) return;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      const { readTextFile } = await import('@tauri-apps/plugin-fs');

      // Listen for Tauri's native file drop events
      this.unlistenFileDrop = await listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
        const paths = event.payload.paths;
        if (paths && paths.length > 0) {
          try {
            const content = await readTextFile(paths[0]);
            this.messageContent = content;
            this.isDragOver = false;
          } catch (error) {
            console.error('Failed to read dropped file:', error);
          }
        }
      });

      // Listen for drag enter/leave for visual feedback
      await listen('tauri://drag-enter', () => {
        this.isDragOver = true;
      });

      await listen('tauri://drag-leave', () => {
        this.isDragOver = false;
      });
    } catch (error) {
      console.error('Failed to setup drag-drop listener:', error);
    }
  }

  async loadConfig() {
    try {
      this.config = await this.tauriService.getConfig();
    } catch (error) {
      console.error('Failed to load config:', error);
    }
  }

  async saveConfig() {
    try {
      await this.tauriService.saveConfig(this.config);
      this.showSettings = false;
      // Re-check connection after config change
      this.checkConnectionWithSpinner();
    } catch (error) {
      console.error('Failed to save config:', error);
    }
  }

  async checkConnectionWithSpinner() {
    // Connection check with minimum 800ms spinner for smooth UX
    this.connectionStatus = 'testing';
    
    const startTime = Date.now();
    let result: 'connected' | 'error' = 'error';
    
    try {
      await this.tauriService.testConnection(10);
      result = 'connected';
    } catch (error) {
      result = 'error';
    }
    
    // Ensure minimum 800ms spinner duration
    const elapsed = Date.now() - startTime;
    if (elapsed < 800) {
      await this.delay(800 - elapsed);
    }
    
    this.connectionStatus = result;
  }

  async testConnection() {
    if (this.isTesting) return;
    
    this.isTesting = true;
    this.isLoading = true;
    this.testingCancelled = false;
    this.connectionStatus = 'testing'; // Clear previous status, show spinner
    
    const startTime = Date.now();
    let result: 'connected' | 'error' = 'error';
    
    try {
      await this.tauriService.testConnection(10); // 10 second timeout for remote/SASL brokers
      if (!this.testingCancelled) {
        result = 'connected';
      }
    } catch (error) {
      if (!this.testingCancelled) {
        result = 'error';
        console.error('Connection test failed:', error);
      }
    }
    
    // Ensure minimum 800ms spinner duration
    const elapsed = Date.now() - startTime;
    if (elapsed < 800 && !this.testingCancelled) {
      await this.delay(800 - elapsed);
    }
    
    if (!this.testingCancelled) {
      this.connectionStatus = result;
    }
    
    this.isTesting = false;
    this.isLoading = false;
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  cancelTestConnection() {
    this.testingCancelled = true;
    this.isTesting = false;
    this.isLoading = false;
    this.connectionStatus = 'unknown';
  }

  async sendMessage() {
    if (!this.messageContent.trim() || this.isSending) return;

    const entry: MessageEntry = {
      id: this.tauriService.generateId(),
      content: this.messageContent,
      timestamp: new Date(),
      status: 'pending'
    };

    // Add to history immediately
    this.messages.unshift(entry);
    this.isSending = true;

    try {
      // Wrap send in a timeout to prevent infinite hanging
      const sendPromise = this.tauriService.sendMessage(this.messageContent);
      const timeoutPromise = new Promise<never>((_, reject) => 
        setTimeout(() => reject(new Error('Send timeout: Kafka server unreachable')), 10000)
      );
      
      await Promise.race([sendPromise, timeoutPromise]);
      entry.status = 'success';
      this.messageContent = '';
      // Mark as connected if send succeeds
      this.connectionStatus = 'connected';
    } catch (error: any) {
      entry.status = 'error';
      entry.errorMessage = error.message || 'Failed to send message';
      this.connectionStatus = 'error';
    } finally {
      this.isSending = false;
    }
  }

  async attachFile() {
    if (!isTauri()) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const { readTextFile } = await import('@tauri-apps/plugin-fs');

      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Text Files',
          extensions: ['txt', 'json', 'xml', 'csv', 'log', 'md', 'yaml', 'yml']
        }, {
          name: 'All Files',
          extensions: ['*']
        }]
      });
      
      if (selected && typeof selected === 'string') {
        const content = await readTextFile(selected);
        this.messageContent = content;
      }
    } catch (error) {
      console.error('Failed to read file:', error);
    }
  }

  async browseCertFile(field: 'ssl_ca_cert_path' | 'ssl_client_cert_path' | 'ssl_client_key_path') {
    if (!isTauri()) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');

      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Certificate Files',
          extensions: ['pem', 'crt', 'cert', 'key', 'p12']
        }, {
          name: 'All Files',
          extensions: ['*']
        }]
      });

      if (selected && typeof selected === 'string') {
        this.config[field] = selected;
      }
    } catch (error) {
      console.error('Failed to select certificate file:', error);
    }
  }

  getSecurityLabel(): string {
    switch (this.config.security_protocol) {
      case 'Ssl': return 'SSL';
      case 'SaslPlaintext': return 'SASL';
      case 'SaslSsl': return 'SASL+SSL';
      default: return 'PLAIN';
    }
  }

  // --- Create Topic ---
  async createTopic() {
    if (!this.newTopicName.trim() || this.isCreatingTopic) return;

    this.isCreatingTopic = true;
    this.topicCreateStatus = 'none';
    this.topicCreateMessage = '';

    try {
      const result = await this.tauriService.createTopic(
        this.newTopicName.trim(),
        this.newTopicPartitions,
        this.newTopicReplication
      );
      this.topicCreateStatus = 'success';
      this.topicCreateMessage = result.message;
      this.newTopicName = '';
    } catch (error: any) {
      this.topicCreateStatus = 'error';
      this.topicCreateMessage = error.message || 'Failed to create topic';
    } finally {
      this.isCreatingTopic = false;
    }
  }

  // --- Consume Messages ---
  async fetchMessages() {
    if (this.isConsuming) return;

    const topic = this.consumeTopic.trim() || this.config.topic;
    this.isConsuming = true;
    this.consumeError = '';

    try {
      this.consumedMessages = await this.tauriService.consumeMessages(
        topic,
        this.consumeOffset,
        this.consumeMaxMessages
      );
      if (this.consumedMessages.length === 0) {
        this.consumeError = 'No messages found at the specified offset.';
      }
    } catch (error: any) {
      this.consumeError = error.message || 'Failed to consume messages';
      this.consumedMessages = [];
    } finally {
      this.isConsuming = false;
    }
  }

  formatMessageTimestamp(timestampMs: number): string {
    return new Date(timestampMs).toLocaleString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      month: 'short',
      day: 'numeric',
    });
  }

  // --- Message Detail Modal ---
  openMessageDetail(msg: ConsumedMessage) {
    this.selectedMessage = msg;
    this.copiedField = null;
  }

  closeMessageDetail() {
    this.selectedMessage = null;
    this.copiedField = null;
  }

  async copyToClipboard(text: string | null, field: string) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      this.copiedField = field;
      setTimeout(() => {
        if (this.copiedField === field) this.copiedField = null;
      }, 2000);
    } catch {
      console.error('Failed to copy to clipboard');
    }
  }

  // Keep HostListener as fallback for web-based file drop (dev mode)
  @HostListener('dragover', ['$event'])
  onDragOver(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    this.isDragOver = true;
  }

  @HostListener('dragleave', ['$event'])
  onDragLeave(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    this.isDragOver = false;
  }

  @HostListener('drop', ['$event'])
  async onDrop(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    this.isDragOver = false;

    const files = event.dataTransfer?.files;
    if (files && files.length > 0) {
      const file = files[0];
      try {
        const content = await this.readFile(file);
        this.messageContent = content;
      } catch (error) {
        console.error('Failed to read file:', error);
      }
    }
  }

  private readFile(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(reader.error);
      reader.readAsText(file);
    });
  }

  formatTime(date: Date): string {
    return date.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }

  truncateMessage(content: string, maxLength = 100): string {
    if (content.length <= maxLength) return content;
    return content.substring(0, maxLength) + '...';
  }

  clearHistory() {
    this.messages = [];
  }
}
