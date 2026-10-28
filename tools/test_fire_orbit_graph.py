"""Guard the vendor pipeline when regenerating the editor-only UV adapter."""
from copy import deepcopy
import unittest

from build_fire_orbit_graph import ROOT, ident, objects


def verify_pipeline(source, derived):
    graph = derived[0]
    by_id = {x['m_ObjectId']: x for x in derived}
    alpha = next(x['m_ObjectId'] for x in source if x.get('m_Name') == 'SurfaceDescription.Alpha')
    # Every vendor node, property, slot and target retains its serialized values.
    for obj in source[1:]:
        assert by_id[obj['m_ObjectId']] == obj, obj['m_ObjectId']
    for original in source[0]['m_Edges']:
        expected = deepcopy(original)
        if expected['m_InputSlot']['m_Node']['m_Id'] == alpha:
            expected['m_InputSlot'] = {'m_Node': {'m_Id': ident('Coverage')}, 'm_SlotId': 0}
        assert expected in graph['m_Edges'], expected
    bell = by_id[ident('BellStripProperty')]
    assert bell['m_Value'] == 0, 'orbit must not enable Bell UV flow'
    source_ids = {x['m_ObjectId']: x for x in source}
    uv_edges = [e for e in graph['m_Edges']
                if e['m_OutputSlot']['m_Node']['m_Id'] == ident('FlowUV')
                and e['m_InputSlot']['m_Node']['m_Id'] in source_ids]
    assert uv_edges, 'no vendor UV inputs adapted'
    for edge in uv_edges:
        node = source_ids[edge['m_InputSlot']['m_Node']['m_Id']]
        slot = next(source_ids[s['m_Id']] for s in node['m_Slots']
                    if source_ids[s['m_Id']]['m_Id'] == edge['m_InputSlot']['m_SlotId'])
        assert slot['m_Type'].endswith('.UVMaterialSlot') and slot['m_Channel'] == 0
    assert any(e['m_OutputSlot']['m_Node']['m_Id'] == ident('FlowWindow')
               and e['m_InputSlot']['m_Node']['m_Id'] == alpha for e in graph['m_Edges'])


class FireGraphTests(unittest.TestCase):
    def setUp(self):
        self.source = objects(ROOT / 'Editor/Vfx/References/MaykerStudio/FreeSlashVFX/Shaders/Slash.shadergraph')
        self.derived = objects(ROOT / 'Editor/Vfx/Suzu/Shaders/FireOrbitCoverage.shadergraph')

    def test_vendor_pipeline_retained(self):
        verify_pipeline(self.source, self.derived)

    def test_changed_vendor_node_rejected(self):
        self.derived[1]['m_Name'] = 'changed'
        with self.assertRaises(AssertionError):
            verify_pipeline(self.source, self.derived)

    def test_disconnected_vendor_edge_rejected(self):
        self.derived[0]['m_Edges'].remove(self.source[0]['m_Edges'][0])
        with self.assertRaises(AssertionError):
            verify_pipeline(self.source, self.derived)

    def test_orbit_opted_into_bell_rejected(self):
        next(x for x in self.derived if x['m_ObjectId'] == ident('BellStripProperty'))['m_Value'] = 1
        with self.assertRaises(AssertionError):
            verify_pipeline(self.source, self.derived)


if __name__ == '__main__':
    unittest.main()
