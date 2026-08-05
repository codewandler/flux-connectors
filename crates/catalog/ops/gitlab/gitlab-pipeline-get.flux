op gitlab-pipeline-get(project_id: Number, pipeline_id: Number) -> Any
  description "Get one CI/CD pipeline's status and timing by its id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/projects/{project_id}/pipelines/{pipeline_id}")
  response = http.request(method: "GET", url)
  return response
