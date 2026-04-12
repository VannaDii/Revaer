{{- define "revaer.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "revaer.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := include "revaer.name" . -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "revaer.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "revaer.labels" -}}
helm.sh/chart: {{ include "revaer.chart" . }}
app.kubernetes.io/name: {{ include "revaer.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "revaer.selectorLabels" -}}
app.kubernetes.io/name: {{ include "revaer.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "revaer.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "revaer.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "revaer.databaseSecretName" -}}
{{- if .Values.database.existingSecret -}}
{{- .Values.database.existingSecret -}}
{{- else -}}
{{- printf "%s-db" (include "revaer.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "revaer.databaseSecretChecksum" -}}
{{- if and (not .Values.database.existingSecret) .Values.database.url -}}
{{ include (print $.Template.BasePath "/secret.yaml") . | sha256sum }}
{{- end -}}
{{- end -}}
