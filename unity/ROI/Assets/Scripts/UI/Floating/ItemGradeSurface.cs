using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // TBH-inspired filled rarity tiles, using ROI's palette and retained WuWa UI geometry.
    // One existing quad; no textures, extra canvases, shadow meshes or per-frame updates.
    [RequireComponent(typeof(Graphic))]
    public sealed class ItemGradeSurface : BaseMeshEffect
    {
        static readonly Color Empty = new(.065f,.08f,.105f,1);
        Color _top=Empty, _bottom=Empty;
        public static Color Accent(ItemView item,bool gem=false)
        {
            if(item==null)return new Color(.32f,.35f,.39f);
            if(item.grade_step==8)return new Color(.99f,.72f,.85f,1);
            // Only the final tier is Eternal. Tier 9 keeps its own dark-violet identity after
            // the rarity ladder was shifted so Tier 2 starts at green.
            if(item.grade_step==9)return new Color(.78f,.26f,.66f,1);
            return gem?GameText.GradeColor(item.grade_step):ItemSlot.ItemColor(item);
        }
        public static Color TextAccent(ItemView item,bool gem=false)
        {
            var tone=Accent(item,gem);
            // Names sit on a dark caption strip. Lift the grade colour so the name remains
            // legible while preserving the same hue used by the border and tile.
            return Color.Lerp(tone,Color.white,item!=null&&item.grade_step==9?.42f:.30f);
        }
        public void Bind(ItemView item,bool gem=false,bool selected=false)
        {
            var tone=Accent(item,gem);
            bool common=item!=null&&(gem||item.kind=="Weapon"||item.kind=="Armor")&&item.grade_step==0;
            var top=item==null?Empty: item.grade_step==9?new Color(.035f,.025f,.075f,1)
                :item.grade_step==8?new Color(.22f,.20f,.29f,1):Color.Lerp(Empty,tone,common?.08f:.12f);
            var bottom=item==null?Empty: item.grade_step==9?new Color(.30f,.045f,.30f,1)
                :item.grade_step==8?new Color(.39f,.24f,.33f,1):Color.Lerp(Empty,tone,common?.16f:.28f);
            // Selection keeps the rarity hue. A separate marker identifies the picked material.
            if(selected){top=Color.Lerp(top,Color.white,.08f);bottom=Color.Lerp(bottom,Color.white,.08f);}
            if(top==_top&&bottom==_bottom)return;
            _top=top;_bottom=bottom;graphic.SetVerticesDirty();
        }
        public override void ModifyMesh(VertexHelper mesh)
        {
            if(!IsActive())return;
            var rect=((RectTransform)transform).rect;
            var vertex=new UIVertex();
            for(int i=0;i<mesh.currentVertCount;i++)
            {
                mesh.PopulateUIVertex(ref vertex,i);
                float y=Mathf.InverseLerp(rect.yMin,rect.yMax,vertex.position.y);
                vertex.color=(Color)vertex.color*Color.Lerp(_bottom,_top,y);
                mesh.SetUIVertex(vertex,i);
            }
        }
    }
}
