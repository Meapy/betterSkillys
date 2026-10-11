package com.company.assembleegameclient.util
{
   import flash.display.BitmapData;

   // TextureRedrawer.redrawFace's shaded copies of one image, by shade (a wall has a few).
   public class FaceShades
   {
      private var shades_:Vector.<Number> = new Vector.<Number>();
      private var images_:Vector.<BitmapData> = new Vector.<BitmapData>();

      public function FaceShades()
      {
         super();
      }

      public function find(shade:Number) : BitmapData
      {
         for(var i:int = 0; i < this.shades_.length; i++)
         {
            if(this.shades_[i] == shade)
            {
               return this.images_[i];
            }
         }
         return null;
      }

      public function add(shade:Number, image:BitmapData) : void
      {
         this.shades_.push(shade);
         this.images_.push(image);
      }
   }
}
