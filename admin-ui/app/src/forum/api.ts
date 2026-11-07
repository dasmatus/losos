/**
 * Forum API client (Phase 1, scaffold-gated). Uses admin bearer-token auth
 * via lib/api call(). Backend: /api/forum/* (guarded_as). Shapes match
 * backend/src/forum/models.rs and http.rs handlers.
 */
import { call } from "@/lib/api";

export type ForumId = number;

export interface ForumStatus {
  enabled: boolean;
  implemented?: boolean;
  phase?: number;
  storageReady?: boolean;
  edgeLocalRouting?: boolean;
  activityPub?: boolean;
  matrixDmBridge?: boolean;
  assistant?: boolean;
  scaffold?: boolean;
  scaffoldConfig?: {
    stateDir?: string;
    port?: number;
    apiPath?: string;
    uiPath?: string;
    edgeLocalRouting?: boolean;
    activityPubEnabled?: boolean;
    activityPubImplemented?: boolean;
    matrixDmBridgeEnabled?: boolean;
    matrixDmBridgeImplemented?: boolean;
    assistantEnabled?: boolean;
  };
  design?: string;
  [key: string]: unknown;
}

export interface Space {
  id: ForumId;
  slug: string;
  name: string;
  description: string;
  createdBy: string;
  createdAt: string;
}

export type ChannelKind = "topic" | "channel" | "dm";

export interface Channel {
  id: ForumId;
  spaceId: ForumId | null;
  slug: string;
  name: string;
  kind: ChannelKind;
  createdBy: string;
  createdAt: string;
}

export interface Topic {
  id: ForumId;
  channelId: ForumId;
  title: string;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
  closed: boolean;
}

export interface Post {
  id: ForumId;
  topicId: ForumId;
  author: string;
  body: string;
  createdAt: string;
  updatedAt: string;
}

export interface Message {
  id: ForumId;
  channelId: ForumId;
  threadId: ForumId | null;
  author: string;
  body: string;
  createdAt: string;
}

export interface Mention {
  target: string;
  isAssistant: boolean;
}

export interface CreateTopicResult {
  topic: Topic;
  post: Post;
  mentions: Mention[];
}

export interface CreateMessageResult {
  message: Message;
  mentions: Mention[];
}

export const forum = {
  status: () => call<ForumStatus>("/api/forum"),
  listSpaces: () => call<Space[]>("/api/forum/spaces"),
  createSpace: (body: { slug: string; name: string; description?: string }) =>
    call<Space>("/api/forum/spaces", { method: "POST", contentType: "application/json", body: JSON.stringify(body) }),
  listChannels: (spaceId?: ForumId) =>
    call<Channel[]>(spaceId == null ? "/api/forum/channels" : `/api/forum/channels?spaceId=${spaceId}`),
  createChannel: (body: { spaceId?: ForumId | null; slug: string; name: string; kind: ChannelKind }) =>
    call<Channel>("/api/forum/channels", { method: "POST", contentType: "application/json", body: JSON.stringify(body) }),
  listTopics: (channelId?: ForumId) =>
    call<Topic[]>(channelId == null ? "/api/forum/topics" : `/api/forum/topics?channelId=${channelId}`),
  createTopic: (body: { channelId: ForumId; title: string; body: string }) =>
    call<CreateTopicResult>("/api/forum/topics", { method: "POST", contentType: "application/json", body: JSON.stringify(body) }),
  listPosts: (topicId: ForumId) => call<Post[]>(`/api/forum/topics/${topicId}/posts`),
  createPost: (topicId: ForumId, body: { body: string }) =>
    call<{ post: Post; mentions: Mention[] }>(`/api/forum/topics/${topicId}/posts`, { method: "POST", contentType: "application/json", body: JSON.stringify({ topicId, body: body.body }) }),
  listMessages: (channelId: ForumId) => call<Message[]>(`/api/forum/channels/${channelId}/messages`),
  createMessage: (channelId: ForumId, body: { body: string; threadId?: ForumId | null }) =>
    call<CreateMessageResult>(`/api/forum/channels/${channelId}/messages`, { method: "POST", contentType: "application/json", body: JSON.stringify({ channelId, body: body.body, threadId: body.threadId ?? null }) }),
};
