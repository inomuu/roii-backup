using System;
using System.Linq;
using System.Text;

namespace Roi.UI
{
    // Complete presentation of host-projected values. No stat rolls or equip formulas here.
    public static class FloatingItemDescription
    {
        [Serializable] sealed class Metadata { public string[] wieldable,special_vals; }
        public static string Format(ItemView it,ExemplarEquipmentActions.Row row=null,bool gem=false,bool inspector=false)
        {
            if(it==null)return null;
            var extra=ForgeModel.Parse<Metadata>(row?.Raw);
            var s=new StringBuilder(inspector?"":ForgeUi.Name(it));
            bool gear=it.kind=="Weapon"||it.kind=="Armor";
            if(!inspector)s.Append('\n').Append(GameText.GradeLabel(it.grade_step)).Append(" · ");
            s.Append(gem?Loc.Get("UI_FORGE_GEMS"):Loc.GetOr("UI_FLOAT_KIND_"+it.kind,it.kind));
            if(gear&&it.slot!=it.kind)s.Append(" · ").Append(Loc.GetOr("UI_SLOT_"+it.slot,it.slot));
            if(it.level_req>0)s.Append('\n').Append(Loc.Get("UI_EX_GEAR_REQUIRED_LEVEL")).Append(' ').Append(it.level_req);
            if(gear&&it.weapon_type!="None"&&!string.IsNullOrEmpty(it.weapon_type))s.Append('\n').Append(Loc.Fmt("UI_FLOAT_WEAPON_TYPE",Loc.GetOr("UI_WEAPON_"+it.weapon_type.ToUpperInvariant(),it.weapon_type)));
            var units=row?.Wieldable??it.wieldable??extra?.wieldable;
            if(gear&&units!=null)s.Append('\n').Append(Loc.Fmt("UI_FLOAT_WIELDABLE",units.Length==0?Loc.Get("UI_FLOAT_ALL_CHARACTERS"):string.Join(", ",units.Select(GameText.UnitName))));
            if((gear||it.kind=="Card")&&!string.IsNullOrEmpty(it.element))s.Append('\n').Append(Loc.Get("UI_SKD_ELEMENT")).Append(": ").Append(GameText.ElementName(it.element));
            if(it.bound)s.Append('\n').Append(Loc.Get("UI_ITEM_BOUND"));
            if(it.unique)s.Append('\n').Append(Loc.Get("UI_FLOAT_UNIQUE"));
            if(row?.OwnerId>0)s.Append('\n').Append(Loc.Fmt("UI_FLOAT_OWNER",GameText.UnitName(row.OwnerUnitDef)));
            if(it.equipped_by>0&&row?.OwnerId<=0&&!string.IsNullOrEmpty(row?.OwnerUnitDef))s.Append('\n').Append(Loc.Fmt("UI_FLOAT_OWNER",GameText.UnitName(row?.OwnerUnitDef??"")));
            Stat(s,"UI_EXEMPLAR_ATK",it.graded_atk);Stat(s,"UI_EXEMPLAR_MATK",it.graded_matk);Stat(s,"UI_EXEMPLAR_DEF",it.graded_def);
            if(it.graded_atk_speed_bp!=0)s.Append('\n').Append(Loc.Fmt("UI_FLOAT_ATTACK_SPEED",(it.graded_atk_speed_bp/100f).ToString("0.##")));
            Lines(s,"UI_FLOAT_AUTHORED",it.authored_lines);
            if(it.bonus_lines?.Length>0)
            {
                s.Append("\n\n").Append(Loc.Get("UI_FLOAT_BONUSES"));
                for(int i=0;i<it.bonus_lines.Length;i++)s.Append('\n').Append(it.bonus_lines[i]).Append(i==it.locked_slot?" · "+Loc.Get("UI_FLOAT_LOCKED"):"");
            }
            Lines(s,"UI_FLOAT_CARD_EFFECTS",it.card_lines);
            Lines(s,it.foil?"UI_FLOAT_FOIL":"UI_FLOAT_FOIL_INACTIVE",it.card_foil_lines);
            Lines(s,"UI_FLOAT_BIND_EFFECTS",it.bind_lines);Lines(s,"UI_FLOAT_OVERLAY_EFFECTS",it.overlay_lines);
            if(it.kind=="Card")
            {
                s.Append('\n').Append(Loc.Fmt("UI_FLOAT_STARS",it.star));
                s.Append('\n').Append(Loc.GetOr("UI_SLOT_"+it.card_slot,it.card_slot));
                string special=Loc.GetOr("CARD_SPECIAL_"+it.def_id,"");
                if(special.Length>0&&(it.special_vals??extra?.special_vals)!=null)
                {try{s.Append('\n').Append(string.Format(special,(it.special_vals??extra.special_vals).Cast<object>().ToArray()));}catch(FormatException){}}
            }
            if(gear&&(it.socket_count>0||it.gem_slots>0||it.gems?.Length>0))
            {
                s.Append("\n\n").Append(Loc.Get("UI_FLOAT_SOCKET_DETAILS"));
                // Cards are fitted on the character, not inside this equipment instance.
                // These rows describe the capacity this piece grants, never card occupancy.
                for(int i=0;i<it.socket_count;i++)s.Append('\n').Append(Loc.Get("UI_FLOAT_CARD_SOCKET_ROW"));
                int used=it.gems?.Length??0;
                for(int i=0;i<used;i++)
                {
                    s.Append('\n').Append(i+1).Append(". ").Append(Loc.GetOr("ITEM_"+it.gems[i].def_id,it.gems[i].def_id));
                    if(it.gem_line_groups!=null&&i<it.gem_line_groups.Length)
                        foreach(var line in it.gem_line_groups[i].lines??Array.Empty<string>())s.Append("\n   ").Append(line);
                }
                for(int i=used;i<it.gem_slots;i++)s.Append('\n').Append(Loc.Get("UI_FLOAT_GEM_SOCKET_ROW"));
            }
            if(gear||gem)s.Append("\n\n").Append(Loc.Get(it.market_clean&&it.marketable?"UI_FLOAT_MARKET_CLEAN":"UI_FLOAT_MARKET_NOT_CLEAN"));
            string lore=Loc.GetOr("ITEM_LORE_"+it.def_id,"");if(lore.Length>0)s.Append("\n\n").Append(lore);
            return s.ToString();
        }
        static void Stat(StringBuilder s,string key,int n){if(n!=0)s.Append('\n').Append(Loc.Get(key)).Append("  ").Append(n);}
        static void Lines(StringBuilder s,string key,string[] lines)
        {if(lines==null||lines.Length==0)return;s.Append("\n\n").Append(Loc.Get(key));foreach(var line in lines)s.Append('\n').Append(line);}
    }
}
