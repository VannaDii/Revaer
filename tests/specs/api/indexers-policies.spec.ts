import { test, expect } from '../../fixtures/api';

test.describe('Indexer policies', () => {
  test('creates and manages policy sets and rules', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const displayName = `E2E Policy Set ${suffix}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/policies', {
        body: { display_name: displayName, scope: 'global', enabled: true },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/policies', {
      body: { display_name: displayName, scope: 'global', enabled: true },
    });
    expect(created.response.status).toBe(201);
    const policySetId = created.data?.policy_set_public_id;
    if (!policySetId) {
      throw new Error('Missing policy_set_public_id');
    }

    const updated = await api.PATCH('/v1/indexers/policies/{policy_set_public_id}', {
      params: { path: { policy_set_public_id: policySetId } },
      body: { display_name: `${displayName} Updated` },
    });
    expect(updated.response.ok).toBeTruthy();

    const listed = await api.GET('/v1/indexers/policies');
    expect(listed.response.status).toBe(200);
    expect(
      listed.data?.policy_sets.some(
        (policySet) =>
          policySet.policy_set_public_id === policySetId &&
          policySet.display_name === `${displayName} Updated`
      )
    ).toBeTruthy();

    const enabled = await api.POST('/v1/indexers/policies/{policy_set_public_id}/enable', {
      params: { path: { policy_set_public_id: policySetId } },
    });
    expect(enabled.response.status).toBe(204);

    const disabled = await api.POST('/v1/indexers/policies/{policy_set_public_id}/disable', {
      params: { path: { policy_set_public_id: policySetId } },
    });
    expect(disabled.response.status).toBe(204);

    const reordered = await api.POST('/v1/indexers/policies/reorder', {
      body: { ordered_policy_set_public_ids: [policySetId] },
    });
    expect(reordered.response.status).toBe(204);

    const ruleCreated = await api.POST(
      '/v1/indexers/policies/{policy_set_public_id}/rules',
      {
        params: { path: { policy_set_public_id: policySetId } },
        body: {
          rule_type: 'block_title_regex',
          match_field: 'title',
          match_operator: 'regex',
          sort_order: 10,
          match_value_text: 'sample',
          action: 'drop_canonical',
          severity: 'hard',
          is_case_insensitive: true,
          rationale: 'e2e test',
        },
      }
    );
    expect(ruleCreated.response.status).toBe(201);
    const ruleId = ruleCreated.data?.policy_rule_public_id;
    if (!ruleId) {
      throw new Error('Missing policy_rule_public_id');
    }

    const listedWithRule = await api.GET('/v1/indexers/policies');
    expect(listedWithRule.response.status).toBe(200);
    const listedPolicySet = listedWithRule.data?.policy_sets.find(
      (policySet) => policySet.policy_set_public_id === policySetId
    );
    expect(listedPolicySet?.rules.some((rule) => rule.policy_rule_public_id === ruleId)).toBe(
      true
    );

    const ruleEnabled = await api.POST(
      '/v1/indexers/policies/rules/{policy_rule_public_id}/enable',
      {
        params: { path: { policy_rule_public_id: ruleId } },
      }
    );
    expect(ruleEnabled.response.status).toBe(204);

    const ruleDisabled = await api.POST(
      '/v1/indexers/policies/rules/{policy_rule_public_id}/disable',
      {
        params: { path: { policy_rule_public_id: ruleId } },
      }
    );
    expect(ruleDisabled.response.status).toBe(204);

    const ruleReordered = await api.POST(
      '/v1/indexers/policies/{policy_set_public_id}/rules/reorder',
      {
        params: { path: { policy_set_public_id: policySetId } },
        body: { ordered_policy_rule_public_ids: [ruleId] },
      }
    );
    expect(ruleReordered.response.status).toBe(204);

    const invalidExpiry = await api.POST(
      '/v1/indexers/policies/{policy_set_public_id}/rules',
      {
        params: { path: { policy_set_public_id: policySetId } },
        body: {
          rule_type: 'block_title_regex',
          match_field: 'title',
          match_operator: 'regex',
          sort_order: 20,
          match_value_text: 'sample',
          action: 'drop_canonical',
          severity: 'hard',
          expires_at: 'not-a-date',
        },
      }
    );
    expect(invalidExpiry.response.status).toBe(400);
  });
});
