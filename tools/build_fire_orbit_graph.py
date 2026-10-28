"""Derive an editor-only alpha/UV variant; leave the licensed vendor graph intact."""
from copy import deepcopy
import json
from pathlib import Path
from uuid import NAMESPACE_URL, uuid5

ROOT = Path(__file__).resolve().parents[1] / 'unity/ROI/Assets'


def objects(path):
    text = path.read_text()
    decoder = json.JSONDecoder()
    result = []
    while text.strip():
        text = text.lstrip()
        value, end = decoder.raw_decode(text)
        result.append(value)
        text = text[end:]
    return result


def ident(name):
    return uuid5(NAMESPACE_URL, 'roi/fire-orbit/' + name).hex


def main():
    data = objects(ROOT / 'Editor/Vfx/References/MaykerStudio/FreeSlashVFX/Shaders/Slash.shadergraph')
    graph = data[0]
    originals = deepcopy(data)

    def template(kind):
        return deepcopy(next(x for x in originals if x['m_Type'].endswith('.' + kind)))

    def add_slot(kind, name, number, output=False):
        slot = template(kind + 'MaterialSlot')
        slot.update(m_ObjectId=ident(name), m_Id=number, m_DisplayName=name,
                    m_ShaderOutputName=name, m_SlotType=int(output), m_StageCapability=2)
        data.append(slot)
        return {'m_Id': slot['m_ObjectId']}

    def add_node(node):
        node['m_Group'] = {'m_Id': ''}
        data.append(node)
        graph['m_Nodes'].append({'m_Id': node['m_ObjectId']})
        return node['m_ObjectId']

    def edge(a, a_slot, b, b_slot):
        return {'m_OutputSlot': {'m_Node': {'m_Id': a}, 'm_SlotId': a_slot},
                'm_InputSlot': {'m_Node': {'m_Id': b}, 'm_SlotId': b_slot}}

    controls = []
    for name, default in [('WhiteBand', 0), ('Taper', 1), ('BellStrip', 0),
                          ('Head', 0), ('Tail', 1), ('Fade', 1)]:
        prop = template('Vector1ShaderProperty')
        prop.update(m_ObjectId=ident(name + 'Property'), m_Name=name,
                    m_DefaultReferenceName='_' + name, m_OverrideReferenceName='_' + name,
                    m_RefNameGeneratedByDisplayName=name, m_Value=default)
        prop['m_Guid']['m_GuidSerialized'] = str(uuid5(NAMESPACE_URL, name + '/roi/fire-orbit'))
        data.append(prop)
        graph['m_Properties'].append({'m_Id': prop['m_ObjectId']})
        node = template('PropertyNode')
        node.update(m_ObjectId=ident(name + 'Node'), m_Property={'m_Id': prop['m_ObjectId']},
                    m_Slots=[add_slot('Vector1', name + 'Value', 0, True)])
        controls.append(add_node(node))

    uv = template('UVNode')
    uv.update(m_ObjectId=ident('UV0'), m_OutputChannel=0,
              m_Slots=[add_slot('Vector4', 'UVValue', 0, True)])
    uv_id = add_node(uv)
    examples = objects(ROOT / 'TextMesh Pro/Shaders/TMP_SDF-URP Unlit.shadergraph')
    custom = deepcopy(next(x for x in examples if x['m_Type'].endswith('.CustomFunctionNode')))
    custom.update(m_ObjectId=ident('Coverage'), m_Name='Fire Orbit UV Coverage',
                  m_FunctionName='FireOrbitCoverage', m_FunctionSource='e843f643de7e44a699f40ce4f7ef0c83',
                  m_SourceType=0, m_FunctionBody='', m_Slots=[
                      add_slot('Vector1', 'Alpha', 0), add_slot('Vector4', 'UV', 1),
                      add_slot('Vector1', 'WhiteBand', 2), add_slot('Vector1', 'Taper', 3),
                      add_slot('Vector1', 'Out', 4, True), add_slot('Vector1', 'BellStrip', 5)])
    custom_id = add_node(custom)
    alpha = next(x for x in originals if x.get('m_Name') == 'SurfaceDescription.Alpha')['m_ObjectId']
    old = next(e for e in graph['m_Edges'] if e['m_InputSlot']['m_Node']['m_Id'] == alpha)
    old['m_InputSlot'] = {'m_Node': {'m_Id': custom_id}, 'm_SlotId': 0}
    # Remap only UV0 at the entrance to the original graph. UV1 carries the
    # vendor's particle custom data and must never pass through this adapter.
    flow_uv = deepcopy(custom)
    flow_uv.update(m_ObjectId=ident('FlowUV'), m_Name='Bell Carrier UV Adapter',
                   m_FunctionName='FireSlashUV', m_Slots=[
                       add_slot('Vector4', 'CarrierUV', 0), add_slot('Vector1', 'UVBell', 1),
                       add_slot('Vector1', 'UVHead', 2), add_slot('Vector1', 'UVTail', 3),
                       add_slot('Vector4', 'VendorUV', 4, True)])
    flow_uv_id = add_node(flow_uv)
    graph['m_Edges'].extend([edge(uv_id, 0, flow_uv_id, 0), edge(controls[2], 0, flow_uv_id, 1),
                            edge(controls[3], 0, flow_uv_id, 2), edge(controls[4], 0, flow_uv_id, 3)])
    original_ids = {x['m_ObjectId']: x for x in originals}
    connected = {(e['m_InputSlot']['m_Node']['m_Id'], e['m_InputSlot']['m_SlotId'])
                 for e in originals[0]['m_Edges']}
    for node_ref in originals[0]['m_Nodes']:
        node = original_ids[node_ref['m_Id']]
        for slot_ref in node.get('m_Slots', []):
            slot = original_ids[slot_ref['m_Id']]
            if (slot['m_Type'].endswith('.UVMaterialSlot') and slot['m_Channel'] == 0
                    and (node['m_ObjectId'], slot['m_Id']) not in connected):
                graph['m_Edges'].append(edge(flow_uv_id, 4, node['m_ObjectId'], slot['m_Id']))

    window = deepcopy(custom)
    window.update(m_ObjectId=ident('FlowWindow'), m_Name='Bell Travel Window',
                  m_FunctionName='FireSlashWindow', m_Slots=[
                      add_slot('Vector1', 'WindowAlpha', 0), add_slot('Vector4', 'WindowUV', 1),
                      add_slot('Vector1', 'WindowBell', 2), add_slot('Vector1', 'WindowHead', 3),
                      add_slot('Vector1', 'WindowTail', 4), add_slot('Vector1', 'WindowFade', 5),
                      add_slot('Vector1', 'WindowOut', 6, True)])
    window_id = add_node(window)
    graph['m_Edges'].extend([edge(flow_uv_id, 4, custom_id, 1), edge(controls[0], 0, custom_id, 2),
                            edge(controls[1], 0, custom_id, 3), edge(controls[2], 0, custom_id, 5),
                            edge(custom_id, 4, window_id, 0), edge(uv_id, 0, window_id, 1),
                            edge(controls[2], 0, window_id, 2), edge(controls[3], 0, window_id, 3),
                            edge(controls[4], 0, window_id, 4), edge(controls[5], 0, window_id, 5),
                            edge(window_id, 6, alpha, 0)])
    graph['m_Path'] = 'ROI/Study'
    output = ROOT / 'Editor/Vfx/Suzu/Shaders/FireOrbitCoverage.shadergraph'
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text('\n\n'.join(json.dumps(x, indent=4) for x in data) + '\n')
    print(output)


if __name__ == '__main__':
    main()
