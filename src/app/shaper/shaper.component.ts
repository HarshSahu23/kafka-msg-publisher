import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { KafkaConfigDialogComponent } from '../shared/kafka-config-dialog/kafka-config-dialog.component';
import { KafkaConfig } from '../services/tauri.service';

function emptyConfig(): KafkaConfig {
  return {
    broker: '',
    topic: '',
    client_id: 'traffic-shaper',
    security_protocol: 'Plaintext',
    sasl_mechanism: 'Plain',
    sasl_username: '',
    sasl_password: '',
    ssl_ca_cert_path: '',
    ssl_client_cert_path: '',
    ssl_client_key_path: '',
    ssl_skip_verification: false,
  };
}

@Component({
  selector: 'app-shaper',
  standalone: true,
  imports: [CommonModule, KafkaConfigDialogComponent],
  templateUrl: './shaper.component.html',
  styleUrl: './shaper.component.css',
})
export class ShaperComponent {
  sourceConfig: KafkaConfig = emptyConfig();
  destConfig: KafkaConfig = emptyConfig();

  sourceConfigured = false;
  destConfigured = false;

  showSourceDialog = false;
  showDestDialog = false;

  isSourceConfigured() { return this.sourceConfig.broker.trim() !== ''; }
  isDestConfigured()   { return this.destConfig.broker.trim() !== ''; }

  /** Connection details (all except topic) from a config */
  private connectionOnly(cfg: KafkaConfig): Partial<KafkaConfig> {
    const { topic: _t, ...conn } = cfg;
    return conn;
  }

  onSourceSaved(config: KafkaConfig) {
    this.sourceConfig = config;
    this.sourceConfigured = true;
    this.showSourceDialog = false;
    // Auto-populate dest with same connection (not topic) if dest hasn't been touched yet
    if (!this.destConfigured) {
      this.destConfig = { ...config, topic: this.destConfig.topic };
    }
  }

  onDestSaved(config: KafkaConfig) {
    this.destConfig = config;
    this.destConfigured = true;
    this.showDestDialog = false;
    // Auto-populate source with same connection (not topic) if source hasn't been touched yet
    if (!this.sourceConfigured) {
      this.sourceConfig = { ...config, topic: this.sourceConfig.topic };
    }
  }

  truncate(val: string, max = 22): string {
    return val.length > max ? val.substring(0, max) + '...' : val;
  }
}
