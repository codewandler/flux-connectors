op algolia-object-save(index_name: String, object_id: String, body: Any) -> Any
  description "Save one record at a known object id, creating it if it does not exist and REPLACING it wholesale if it does. This is not a partial update: attributes absent from the body are removed from the stored record. Algolia applies the write asynchronously, so the response's `taskID` is an acknowledgement of acceptance rather than of visibility, and a search run immediately afterwards may still return the old record. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message` in the response body."
  risk "high"
  idempotency "conditional"
  effects ["write", "network"]
  expose true

  base = "https://{app_id}.algolia.net"
  X_Algolia_Application_Id = "{app_id}"
  url = fmt("{base}/1/indexes/{index_name}/{object_id}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "X-Algolia-Application-Id": X_Algolia_Application_Id, "content-type": content_type }, method: "PUT", url)
  return response
