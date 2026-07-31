op contentful-asset-get(asset_id: String) -> Any
  description "Get one published asset (an uploaded file, e.g. an image) by id, with its metadata resolved to one locale"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://cdn.contentful.com/spaces/{space_id}/environments/{environment_id}"
  url = fmt("{base}/assets/{asset_id}")
  response = http.request(method: "GET", url)
  return response
