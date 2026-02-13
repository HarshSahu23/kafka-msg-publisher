import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

/** Security protocol options */
export type SecurityProtocol = 'Plaintext' | 'Ssl' | 'SaslPlaintext' | 'SaslSsl';

/** Kafka configuration */
export interface KafkaConfig {
  broker: string;
  topic: string;
  client_id: string;
  security_protocol: SecurityProtocol;
  sasl_username: string;
  sasl_password: string;
  ssl_ca_cert_path: string;
  ssl_client_cert_path: string;
  ssl_client_key_path: string;
  ssl_skip_verification: boolean;
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

/** Result of topic creation */
export interface TopicCreateResult {
  success: boolean;
  message: string;
  topic: string;
}

/** A consumed message from Kafka */
export interface ConsumedMessage {
  offset: number;
  key: string | null;
  value: string | null;
  timestamp: number;
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
   * Test connection to Kafka broker with timeout
   */
  async testConnection(timeoutSecs: number = 5): Promise<boolean> {
    const result = await invoke<CommandResult<boolean>>('test_kafka_connection', { timeoutSecs });
    
    if (result.type === 'Ok') {
      return result.data as boolean;
    } else {
      throw new Error(result.data as string);
    }
  }

  /**
   * Create a new Kafka topic
   */
  async createTopic(topicName: string, numPartitions: number = 1, replicationFactor: number = 1): Promise<TopicCreateResult> {
    const result = await invoke<CommandResult<TopicCreateResult>>('create_kafka_topic', {
      topicName,
      numPartitions,
      replicationFactor,
    });

    if (result.type === 'Ok') {
      return result.data as TopicCreateResult;
    } else {
      throw new Error(result.data as string);
    }
  }

  /**
   * Consume messages from a Kafka topic
   */
  async consumeMessages(topic: string, offset: number = 0, maxMessages: number = 50): Promise<ConsumedMessage[]> {
    const result = await invoke<CommandResult<ConsumedMessage[]>>('consume_kafka_messages', {
      topic,
      offset,
      maxMessages,
    });

    if (result.type === 'Ok') {
      return result.data as ConsumedMessage[];
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
