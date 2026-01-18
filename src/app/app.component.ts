import { Component, OnInit, HostListener, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, KafkaConfig, MessageEntry } from './services/tauri.service';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { open } from '@tauri-apps/plugin-dialog';

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
    client_id: 'kafka-msg-publisher'
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
    try {
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
      await this.tauriService.testConnection(3);
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
      await this.tauriService.testConnection(3); // 3 second timeout
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
    try {
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
