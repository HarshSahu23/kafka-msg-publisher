import { Component, Input, Output, EventEmitter, OnInit } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TauriService, KafkaConfig } from '../../services/tauri.service';

function isTauri(): boolean {
  return !!(window as any).__TAURI_INTERNALS__;
}

@Component({
  selector: 'app-kafka-config-dialog',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './kafka-config-dialog.component.html',
  styleUrl: './kafka-config-dialog.component.css',
})
export class KafkaConfigDialogComponent implements OnInit {
  @Input() config!: KafkaConfig;
  @Input() title: string = 'Kafka Configuration';
  @Output() saved = new EventEmitter<KafkaConfig>();
  @Output() cancelled = new EventEmitter<void>();

  /** Working copy — we never mutate the @Input directly */
  editConfig!: KafkaConfig;
  showSecuritySettings = false;

  // Test connection state (own; does not affect publisher's status bar)
  isTesting = false;
  isLoading = false;
  connectionStatus: 'unknown' | 'connected' | 'error' | 'testing' = 'unknown';
  private testingCancelled = false;

  constructor(private tauriService: TauriService) {}

  ngOnInit() {
    this.editConfig = { ...this.config };
  }

  // ---------- Test connection ----------

  async testConnection() {
    if (this.isTesting) return;
    this.isTesting = true;
    this.isLoading = true;
    this.testingCancelled = false;
    this.connectionStatus = 'testing';

    const start = Date.now();
    let result: 'connected' | 'error' = 'error';

    try {
      await this.tauriService.testConnection(10);
      if (!this.testingCancelled) result = 'connected';
    } catch {
      if (!this.testingCancelled) result = 'error';
    }

    const elapsed = Date.now() - start;
    if (elapsed < 800 && !this.testingCancelled) {
      await this.delay(800 - elapsed);
    }

    if (!this.testingCancelled) this.connectionStatus = result;
    this.isTesting = false;
    this.isLoading = false;
  }

  cancelTestConnection() {
    this.testingCancelled = true;
    this.isTesting = false;
    this.isLoading = false;
    this.connectionStatus = 'unknown';
  }

  private delay(ms: number): Promise<void> {
    return new Promise(r => setTimeout(r, ms));
  }

  // ---------- Cert file picker ----------

  async browseCertFile(field: 'ssl_ca_cert_path' | 'ssl_client_cert_path' | 'ssl_client_key_path') {
    if (!isTauri()) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'Certificate Files', extensions: ['pem', 'crt', 'cert', 'key', 'p12'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      if (selected && typeof selected === 'string') {
        this.editConfig[field] = selected;
      }
    } catch (e) {
      console.error('Failed to select cert file:', e);
    }
  }

  // ---------- Actions ----------

  onOverlayClick(event: MouseEvent) {
    if ((event.target as HTMLElement).classList.contains('dialog-overlay')) {
      this.cancelled.emit();
    }
  }

  onSave() {
    this.saved.emit({ ...this.editConfig });
  }

  onCancel() {
    this.cancelled.emit();
  }
}
