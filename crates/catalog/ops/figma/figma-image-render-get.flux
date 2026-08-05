op figma-image-render-get(file_key: String, ids: String) -> Any
  description "Render one or more nodes to images and return download URLs for them. The URLs point at short-lived storage and expire — Figma does not guarantee they stay valid, so fetch or store the image promptly rather than caching the URL itself. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/images/{file_key}")
  response = http.request(method: "GET", query: { ids }, url)
  return response
