using Ionic.Zlib;
using Newtonsoft.Json;
using System.Collections.Generic;
using Shared.resources;
using Shared.terrain;

namespace WorldServer.core.terrain
{
    public static class JsonMapExporter
    {
        public static string Export(XmlData data, TerrainTile[,] tiles)
        {
            var w = tiles.GetLength(0);
            var h = tiles.GetLength(1);
            var dat = new byte[w * h * 2];
            var i = 0;
            var idxs = new Dictionary<TerrainTile, short>(new TileComparer());
            var dict = new List<Loc>();

            for (var y = 0; y < h; y++)
                for (var x = 0; x < w; x++)
                {
                    var tile = tiles[x, y];

                    if (!idxs.TryGetValue(tile, out short idx))
                    {
                        idxs.Add(tile, idx = (short)dict.Count);
                        dict.Add(new Loc()
                        {
                            ground = data.TileTypeToId[tile.TileId],
                            objs = tile.TileObj == null ? null : new Obj[]
                            {
                                new Obj()
                                {
                                    id = tile.TileObj,
                                    name = tile.Name ?? null
                                }
                            },
                            regions = null
                        });
                    }

                    dat[i + 1] = (byte)(idx & 0xff);
                    dat[i] = (byte)(idx >> 8);
                    i += 2;
                }

            return JsonConvert.SerializeObject(new json_dat()
            {
                data = ZlibStream.CompressBuffer(dat),
                width = w,
                height = h,
                dict = dict.ToArray()
            });
        }

        private struct json_dat
        {
            public byte[] data;
            public Loc[] dict;
            public int height;
            public int width;
        }

        private struct Loc
        {
            public string ground;
            public Obj[] objs;
            public Obj[] regions;
        }

        private struct Obj
        {
            public string id;
            public string name;
        }

        private struct TileComparer : IEqualityComparer<TerrainTile>
        {
            public bool Equals(TerrainTile x, TerrainTile y) => x.TileId == y.TileId && x.TileObj == y.TileObj;

            public int GetHashCode(TerrainTile obj) => obj.TileId * 13 + (obj.TileObj == null ? 0 : obj.TileObj.GetHashCode() * obj.Name.GetHashCode() * 29);
        }
    }
}
