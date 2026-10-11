package kabam.rotmg.stage3D.graphic3D
{
   import flash.display.BitmapData;
   import flash.display3D.Context3D;
   import flash.display3D.Context3DTextureFormat;
   import flash.display3D.textures.Texture;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;

   // Small images (sprites, wall faces, tile edges...) packed into a few big textures, so that
   // Renderer can draw consecutive quads with different images in one draw call. Each image is
   // put in a cell the size its own texture would have (rounded up to powers of two, as
   // TextureFactory does, so the quad keeps its shape), with a one-texel border copied from its
   // edges so neighbouring cells never bleed in. Images only go in once they have been drawn for
   // a while (MIN_AGE frames): ones made for a single frame would fill the pages. When every page
   // is full the atlas starts over.
   public class TextureAtlas
   {
      public static const PAGE:int = 1024;
      public static const MAX_CELL:int = 128; // larger images keep their own texture
      private static const PAGES:int = 6;
      private static const MIN_AGE:int = 10;
      private static const point_:Point = new Point();
      private static const rect_:Rectangle = new Rectangle();

      private var ctx_:Context3D;
      private var bitmaps_:Vector.<BitmapData> = new Vector.<BitmapData>();
      private var textures_:Vector.<Texture> = new Vector.<Texture>();
      private var dirty_:Vector.<Boolean> = new Vector.<Boolean>();
      // packing: current page, shelf top, shelf height, x on the shelf
      private var page_:int = 0;
      private var shelfY_:int = 0;
      private var shelfH_:int = 0;
      private var shelfX_:int = 0;
      // image -> its cell: page * 2^24 + x * 2^12 + y (of the image's top-left texel); weak keys
      private var cells_:Dictionary = new Dictionary(true);
      // image -> frame it was first seen
      private var seen_:Dictionary = new Dictionary(true);
      private var frame_:int = 0;

      public function TextureAtlas()
      {
         super();
      }

      public function beginFrame(ctx:Context3D) : void
      {
         if(ctx != this.ctx_)
         {
            // new or recreated GPU context: start over
            this.reset();
            this.ctx_ = ctx;
         }
         this.frame_++;
      }

      // The image's cell (see cells_), or -1 when it isn't (or can't yet be) in the atlas.
      // w and h are its texture size.
      public function cellOf(bmp:BitmapData, w:int, h:int) : int
      {
         var cell:* = this.cells_[bmp];
         if(cell != undefined)
         {
            return int(cell);
         }
         if(w > MAX_CELL || h > MAX_CELL)
         {
            return -1;
         }
         var first:* = this.seen_[bmp];
         if(first == undefined)
         {
            this.seen_[bmp] = this.frame_;
            return -1;
         }
         if(this.frame_ - int(first) < MIN_AGE)
         {
            return -1;
         }
         return this.add(bmp, w, h);
      }

      private function add(bmp:BitmapData, w:int, h:int) : int
      {
         var cw:int = w + 2;
         var ch:int = h + 2;
         if(this.shelfX_ + cw > PAGE)
         {
            this.shelfY_ = this.shelfY_ + this.shelfH_;
            this.shelfX_ = 0;
            this.shelfH_ = 0;
         }
         if(this.shelfY_ + ch > PAGE)
         {
            this.page_++;
            this.shelfY_ = 0;
            this.shelfX_ = 0;
            this.shelfH_ = 0;
            if(this.page_ >= PAGES)
            {
               this.reset();
               return -1;
            }
         }
         while(this.bitmaps_.length <= this.page_)
         {
            this.bitmaps_.push(new BitmapData(PAGE, PAGE, true, 0));
            this.textures_.push(null);
            this.dirty_.push(true);
         }
         var x:int = this.shelfX_ + 1;
         var y:int = this.shelfY_ + 1;
         this.shelfX_ = this.shelfX_ + cw;
         this.shelfH_ = Math.max(this.shelfH_, ch);
         var page:BitmapData = this.bitmaps_[this.page_];
         var bw:int = Math.min(bmp.width, w);
         var bh:int = Math.min(bmp.height, h);
         // the image at the top-left of its texture-sized area, as TextureFactory uploads it
         copy(page, bmp, 0, 0, bw, bh, x, y);
         // borders: the image's edge texels, where the cell's own content ends at the image
         if(bw == w)
         {
            copy(page, page, x + w - 1, y, 1, bh, x + w, y);
         }
         if(bh == h)
         {
            copy(page, page, x, y + h - 1, w + (bw == w ? 1 : 0), 1, x, y + h);
         }
         copy(page, page, x, y, 1, h, x - 1, y);
         copy(page, page, x - 1, y, w + 2, 1, x - 1, y - 1);
         this.dirty_[this.page_] = true;
         var cell:int = this.page_ * 16777216 + x * 4096 + y;
         this.cells_[bmp] = cell;
         return cell;
      }

      private static function copy(dst:BitmapData, src:BitmapData, sx:int, sy:int, w:int, h:int, x:int, y:int) : void
      {
         if(w <= 0 || h <= 0)
         {
            return;
         }
         rect_.x = sx;
         rect_.y = sy;
         rect_.width = w;
         rect_.height = h;
         point_.x = x;
         point_.y = y;
         dst.copyPixels(src, rect_, point_);
      }

      // Uploads the pages that changed this frame; returns their textures (by page).
      public function upload() : Vector.<Texture>
      {
         for(var i:int = 0; i < this.bitmaps_.length; i++)
         {
            if(!this.dirty_[i])
            {
               continue;
            }
            if(this.textures_[i] == null)
            {
               this.textures_[i] = this.ctx_.createTexture(PAGE, PAGE, Context3DTextureFormat.BGRA, false);
            }
            this.textures_[i].uploadFromBitmapData(this.bitmaps_[i]);
            this.dirty_[i] = false;
         }
         return this.textures_;
      }

      public function reset() : void
      {
         for each(var t:Texture in this.textures_)
         {
            if(t != null)
            {
               t.dispose();
            }
         }
         for each(var b:BitmapData in this.bitmaps_)
         {
            b.dispose();
         }
         this.bitmaps_.length = 0;
         this.textures_.length = 0;
         this.dirty_.length = 0;
         this.page_ = 0;
         this.shelfX_ = 0;
         this.shelfY_ = 0;
         this.shelfH_ = 0;
         this.cells_ = new Dictionary(true);
      }
   }
}
