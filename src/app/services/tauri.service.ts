import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

/** Kafka configuration */
export interface KafkaConfig {
  broker: string;
  topic: string;
  client_id: string;
}

/** Result of a message send operation */
export interface SendResult {
  success: boolean;
  message: string;
  timestamp: number;
}

/** Command result wrapper from Rust */
export interface CommandResult<T> {
  type: 'Ok' | 'Err';
  data: T | string;
}

/** Message history entry */
export interface MessageEntry {
  id: string;
  content: string;
  timestamp: Date;
  status: 'pending' | 'success' | 'error';
  errorMessage?: string;
}

@Injectable({
  providedIn: 'root'
})
export class TauriService {
  
  /**
   * Send a message to Kafka
   */
  async sendMessage(message: string): Promise<SendResult> {
    const result = await invoke<CommandResult<SendResult>>('send_kafka_message', { message });
    
    if (result.type === 'Ok') {
      return result.data as SendResult;
    } else {
      throw new Error(result.data as string);
    }
  }

  /**
   * Get the current Kafka configuration
   */
  async getConfig(): Promise<KafkaConfig> {
    return await invoke<KafkaConfig>('get_kafka_config');
  }

  /**
   * Save Kafka configuration
   */
  async saveConfig(config: KafkaConfig): Promise<void> {
    const result = await invoke<CommandResult<void>>('save_kafka_config', { config });
    
    if (result.type === 'Err') {
      throw new Error(result.data as string);
    }
  }

  /**
   * Test connection to Kafka broker
   */
  async testConnection(): Promise<boolean> {
    const result = await invoke<CommandResult<boolean>>('test_kafka_connection');
    
    if (result.type === 'Ok') {
      return result.data as boolean;
    } else {
      throw new Error(result.data as string);
    }
  }

  /**
   * Generate a unique ID for message entries
   */
  generateId(): string {
    return `msg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }
}
