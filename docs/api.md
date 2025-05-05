## API Documentation

This document provides details for the Video Processing API. It covers all available endpoints, request parameters,
response structures, and HTTP status codes.

---
> [!warning]
> This project is under active development. Most of the features have not been implemented yet.

### Base URL

```
https://your-domain.com (default: `http://127.0.0.1`)
```

---

## 1. Health Check

**Endpoint**: `GET /health`

**Description**: Returns the health status and current application version.

#### Response

* **Status Code**: `200 OK`
* **Content-Type**: `application/json`

```json
{
  "status": "ok",
  "version": "1.0.0"
}
```

| Field   | Type   | Description                     |
|---------|--------|---------------------------------|
| status  | string | Health status (`ok` or `error`) |
| version | string | Current application version     |

---

## 2. Publish Video to Pipeline

**Endpoint**: `POST /api/v1/video/pipeline/publish/{id}`

**Description**: Queues a video for processing. Video must not be already waiting or processing.

#### Path Parameters

| Parameter | Type | Description                        |
|-----------|------|------------------------------------|
| `id`      | UUID | Identifier of the video to publish |

#### Responses

* **200 OK**: Video queued successfully.
* **400 Bad Request**: Video already pending or in progress.
* **404 Not Found**: Video not found.

---

## 3. Cancel Video Pipeline

**Endpoint**: `POST /api/v1/video/pipeline/cancel/{id}`

**Description**: Cancels video processing. If processing has started, sends stop signal.

#### Path Parameters

| Parameter | Type | Description                       |
|-----------|------|-----------------------------------|
| `id`      | UUID | Identifier of the video to cancel |

#### Responses

* **200 OK**: Cancellation successful.
* **400 Bad Request**: Video not in waiting or processing state.
* **404 Not Found**: Video not found.

---

## 4. Upload Video

**Endpoint**: `POST /api/v1/video/upload`

**Description**: Uploads a video file along with metadata.

#### Form Data

| Field         | Type   | Required | Description                     |
|---------------|--------|----------|---------------------------------|
| `title`       | string | Yes      | Video title                     |
| `description` | string | No       | Optional video description      |
| `file`        | file   | Yes      | Video file (mp4, mkv, mov, avi) |

#### Responses

* **201 Created**

```json
{
  "id": "<uuid>",
  "title": "My Video",
  "status": "Uploaded"
}
```

* **400 Bad Request**: Missing title or file, unsupported file type.
* **422 Validation Error**: Invalid form data.

---

## 5. Get Video Details

**Endpoint**: `GET /api/v1/video/{id}`

**Description**: Retrieves metadata and processing status for a given video.

#### Path Parameters

| Parameter | Type | Description             |
|-----------|------|-------------------------|
| `id`      | UUID | Identifier of the video |

#### Response

* **200 OK**

```json
{
  "id": "<uuid>",
  "title": "My Video",
  "description": "Optional description",
  "status": "Processing",
  "created_at": "2025-05-05T14:28:00Z"
}
```

* **404 Not Found**: Video not found.

---

## 6. List Videos

**Endpoint**: `POST /api/v1/video/list`

**Description**: Retrieves a list of all videos with their metadata and statuses.

#### Body Parameters

This endpoint currently does not accept filters.

#### Response

* **200 OK**

```json
{
  "total": 2,
  "videos": [
    {
      "id": "<uuid1>",
      "title": "Video One",
      "description": null,
      "status": "Completed",
      "created_at": "2025-05-01T10:00:00Z"
    },
    {
      "id": "<uuid2>",
      "title": "Video Two",
      "description": "Second video",
      "status": "Failed",
      "created_at": "2025-05-02T11:30:00Z"
    }
  ]
}
```

---

## 7. Delete Video

**Endpoint**: `DELETE /api/v1/video/delete/{id}`

**Description**: Deletes a video record and associated file.

#### Path Parameters

| Parameter | Type | Description             |
|-----------|------|-------------------------|
| `id`      | UUID | Identifier of the video |

#### Responses

* **200 OK**: Returns deleted video metadata.

```json
{
  "id": "<uuid>",
  "title": "My Video",
  "description": "Optional description",
  "status": "Uploaded",
  "created_at": "2025-05-05T14:28:00Z"
}
```

* **404 Not Found**: Video not found.

## 8. Real-time Processing Preview

**Description**: While a video is being processed, you can connect to an RTMP stream to view the processing in real
time.

* **Stream URL Format**: `rtmp://{host}/live/stream_{id}`

    * `{host}`: Hostname or IP of the streaming server (default: `localhost`)
    * `{id}`: UUID of the video being processed

**Usage**:

1. Publish a video to the processing pipeline using the Publish endpoint.
2. Open a media player (e.g., VLC) and select "Open Network Stream." Enter the URL above, replacing `{host}` and `{id}`
   with appropriate values.
3. The live preview will automatically stop when video processing completes or is cancelled.

---

## Data Models

### HealthResponse

| Field   | Type   |
|---------|--------|
| status  | string |
| version | string |

### VideoUploadResponse

| Field  | Type   |
|--------|--------|
| id     | UUID   |
| title  | string |
| status | string |

### VideoResponse

| Field       | Type             |
|-------------|------------------|
| id          | UUID             |
| title       | string           |
| description | string?          |
| status      | string           |
| created\_at | string (RFC3339) |

### VideoListResponse

| Field  | Type                   |
|--------|------------------------|
| total  | int                    |
| videos | array of VideoResponse |

---

