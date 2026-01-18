import { Component, OnInit, HostListener } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, KafkaConfig, MessageEntry } from './services/tauri.service';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css',
})
export class AppComponent implements OnInit {
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
  connectionStatus: 'unknown' | 'connected' | 'error' = 'unknown';

  constructor(private tauriService: TauriService) {}

  async ngOnInit() {
    await this.loadConfig();
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
    } catch (error) {
      console.error('Failed to save config:', error);
    }
  }

  async testConnection() {
    this.isLoading = true;
    this.connectionStatus = 'unknown';
    
    try {
      await this.tauriService.testConnection();
      this.connectionStatus = 'connected';
    } catch (error) {
      this.connectionStatus = 'error';
      console.error('Connection test failed:', error);
    } finally {
      this.isLoading = false;
    }
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
      await this.tauriService.sendMessage(this.messageContent);
      entry.status = 'success';
      this.messageContent = '';
    } catch (error: any) {
      entry.status = 'error';
      entry.errorMessage = error.message || 'Failed to send message';
    } finally {
      this.isSending = false;
    }
  }

  // File drop handling
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
