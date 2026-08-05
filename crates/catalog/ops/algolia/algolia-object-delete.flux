op algolia-object-delete(index_name: String, object_id: String) -> Any
  description "Delete one record from an index by its object id. The record is gone with no undo route in the API — restoring it means re-indexing it from the source of truth. Algolia applies the delete asynchronously, so the `taskID` acknowledges acceptance rather than visibility. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{app_id}.algolia.net"
  X_Algolia_Application_Id = "{app_id}"
  url = fmt("{base}/1/indexes/{index_name}/{object_id}")
  response = http.request(headers: { "X-Algolia-Application-Id": X_Algolia_Application_Id }, method: "DELETE", url)
  return response
