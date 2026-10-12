package kabam.rotmg.stage3D
{
   import com.adobe.utils.AGALMiniAssembler;
   import com.company.assembleegameclient.map.Camera;
   import com.company.assembleegameclient.parameters.Parameters;
   import flash.display.BitmapData;
   import flash.display.GraphicsBitmapFill;
   import flash.display.GraphicsEndFill;
   import flash.display.GraphicsPath;
   import flash.display.GraphicsGradientFill;
   import flash.display.IGraphicsData;
   import flash.display.Stage3D;
import flash.display.StageAlign;
import flash.display.StageScaleMode;
   import flash.display3D.Context3D;
   import flash.display3D.Context3DProgramType;
   import flash.display3D.Context3DTextureFormat;
   import flash.display3D.Context3DTriangleFace;
   import flash.display3D.Context3DVertexBufferFormat;
   import flash.display3D.IndexBuffer3D;
   import flash.display3D.Program3D;
   import flash.display3D.VertexBuffer3D;
   import flash.display3D.textures.Texture;
   import flash.geom.Matrix;
   import kabam.rotmg.stage3D.graphic3D.TextureAtlas;
   import flash.geom.ColorTransform;
   import flash.geom.Matrix3D;
   import flash.geom.Vector3D;
   import flash.utils.ByteArray;
   import flash.utils.getTimer;
   import kabam.rotmg.stage3D.Object3D.Object3DStage3D;
   import kabam.rotmg.stage3D.Object3D.Util;
   import kabam.rotmg.stage3D.graphic3D.Graphic3D;
   import kabam.rotmg.stage3D.graphic3D.Program3DFactory;
   import kabam.rotmg.stage3D.graphic3D.TextureFactory;
   import kabam.rotmg.stage3D.proxies.Context3DProxy;
   import org.swiftsuspenders.Injector;

   public class Renderer
   {
      public static const STAGE3D_FILTER_PAUSE:uint = 1;
      public static const STAGE3D_FILTER_BLIND:uint = 2;
      public static const STAGE3D_FILTER_DRUNK:uint = 3;
      private static const POST_FILTER_VERTEX_CONSTANTS:Vector.<Number> = new <Number>[1, 2, 0, 0];
      private static const GRAYSCALE_FRAGMENT_CONSTANTS:Vector.<Number> = new <Number>[0.3, 0.59, 0.11, 0];
      private static const BLIND_FRAGMENT_CONSTANTS:Vector.<Number> = new <Number>[0.05, 0.05, 0.05, 0];
      private static const POST_FILTER_POSITIONS:Vector.<Number> = new <Number>[-1, 1, 0, 0, 1, 1, 1, 0, 1, -1, 1, 1, -1, -1, 0, 1];
      private static const POST_FILTER_TRIS:Vector.<uint> = new <uint>[0, 2, 3, 0, 1, 2];

      public static var inGame:Boolean;

      // Draw calls in the last frame, for GameStatistics.
      public static var drawCalls:int = 0;
      // draw calls by kind in the last frame: batched quads, own vertex buffers, shadows, 3D models
      public static var drawsQuads:int = 0;
      public static var drawsCustom:int = 0;
      public static var drawsShadow:int = 0;
      public static var draws3D:int = 0;
      // Time spent sending the frame to the GPU (renderScene and present), for GameStatistics.
      public static var renderMs:int = 0;

      // Batching (prepareQuads): every plain quad of a frame goes into one vertex buffer, and each
      // run of them with the same image is drawn with one draw call.
      public static var batching:Boolean = true; // debug switch
      private static const BATCH_QUADS:int = 8192;
      private static const ZERO_OFFSET:Vector.<Number> = new <Number>[0, 0, 0, 0];
      // the shadow fragment program's centre, falloff and 1 (see shadowFS)
      private static const SHADOW_CONSTANTS:Vector.<Number> = new <Number>[0.5, 2, 1, 0];
      private var batchCtx_:Context3D;
      private var batchVB_:VertexBuffer3D;
      private var batchIB_:IndexBuffer3D;
      private var batchData_:Vector.<Number> = new Vector.<Number>();
      private var quadOf_:Vector.<int> = new Vector.<int>();
      // per entry: the atlas page its quad's image is on, or -1 (drawn with its own texture)
      private var quadPage_:Vector.<int> = new Vector.<int>();
      private var atlas_:TextureAtlas = new TextureAtlas();
      private var atlasTextures_:Vector.<Texture> = null;
      public static var atlasing:Boolean = true; // debug switch
      // Shadows (GraphicsGradientFill) are batched the same way: their corners are placed on the
      // CPU as Graphic3D.shadowTransform places them, into one vertex buffer.
      private static const BATCH_SHADOWS:int = 2048;
      // Graphic3D's shadow quad: position, colour (the gradient's) and texture coordinates
      private static const SHADOW_X:Vector.<Number> = new <Number>[-0.5, 0.5, -0.5, 0.5];
      private static const SHADOW_Y:Vector.<Number> = new <Number>[0.5, 0.5, -0.5, -0.5];
      private static const SHADOW_U:Vector.<Number> = new <Number>[0, 1, 0, 1];
      private static const SHADOW_V:Vector.<Number> = new <Number>[1, 1, 0, 0];
      private var shadowVB_:VertexBuffer3D;
      private var shadowIB_:IndexBuffer3D;
      private var shadowData_:Vector.<Number> = new Vector.<Number>();
      private var shadowOf_:Vector.<int> = new Vector.<int>();
      private var finalTransform_:Matrix3D = new Matrix3D();

      [Inject]
      public var context3D:Context3DProxy;

      [Inject]
      public var textureFactory:TextureFactory;

      [Inject]
      public var injector:Injector;

      private var tX:Number;

      private var tY:Number;

      public var program2:Program3D;

      private var postProcessingProgram_:Program3D;

      private var blurPostProcessing_:Program3D;

      private var shadowProgram_:Program3D;

      private var graphic3D_:Graphic3D;

      protected var _projection:Matrix3D;

      protected var cameraMatrix_:Matrix3D;

      private var p_:Vector3D;

      private var f_:Vector3D;

      private var u_:Vector3D;

      private var r_:Vector3D;

      private var rd_:Vector.<Number>;

      protected var widthOffset_:Number;

      protected var heightOffset_:Number;

      private var stageWidth:Number = 600;

      private var stageHeight:Number = 600;

      private var sceneTexture_:Texture;

      private var blurFactor:Number = 0.01;

      private var postFilterVertexBuffer_:VertexBuffer3D;

      private var postFilterIndexBuffer_:IndexBuffer3D;

      protected var _vertexShader:String;

      protected var _fragmentShader:String;

      protected var blurFragmentConstants_:Vector.<Number>;

      public function Renderer(render3D:Render3D)
      {
         this.cameraMatrix_ = new Matrix3D();
         this.p_ = new Vector3D();
         this.f_ = new Vector3D();
         this.u_ = new Vector3D();
         this.r_ = new Vector3D();
         this.rd_ = new Vector.<Number>(16,true);
         this._vertexShader = ["m44 op, va0, vc0","m44 v0, va0, vc8","m44 v1, va1, vc8","mov v2, va2"].join("\n");
         this._fragmentShader = ["tex oc, v2, fs0 <2d,clamp>"].join("\n");
         this.blurFragmentConstants_ = Vector.<Number>([0.4,0.6,0.4,1.5]);
         super();
         Renderer.inGame = false;
         this.setTranslationToTitle();
         render3D.add(this.onRender);
      }

      public function init(context3D:Context3D) : void
      {
         this._projection = Util.perspectiveProjection(56,1,0.1,2048);
         var vsAssembler:AGALMiniAssembler = new AGALMiniAssembler();
         vsAssembler.assemble(Context3DProgramType.VERTEX,this._vertexShader);
         var fsAssembler:AGALMiniAssembler = new AGALMiniAssembler();
         fsAssembler.assemble(Context3DProgramType.FRAGMENT,this._fragmentShader);
         this.program2 = context3D.createProgram();
         this.program2.upload(vsAssembler.agalcode,fsAssembler.agalcode);
         var fragSource:String = "tex ft0, v0, fs0 <2d,clamp,linear>\n" + "dp3 ft0.x, ft0, fc0\n" + "mov ft0.y, ft0.x\n" + "mov ft0.z, ft0.x\n" + "mov oc, ft0\n";
         var vertSource:String = "mov op, va0\n" + "add vt0, vc0.xxxx, va0\n" + "div vt0, vt0, vc0.yyyy\n" + "sub vt0.y, vc0.x, vt0.y\n" + "mov v0, vt0\n";
         var assembler:AGALMiniAssembler = new AGALMiniAssembler();
         assembler.assemble(Context3DProgramType.VERTEX,vertSource);
         var vertexShaderAGAL:ByteArray = assembler.agalcode;
         assembler.assemble(Context3DProgramType.FRAGMENT,fragSource);
         var fragmentShaderAGAL:ByteArray = assembler.agalcode;
         this.postProcessingProgram_ = context3D.createProgram();
         this.postProcessingProgram_.upload(vertexShaderAGAL,fragmentShaderAGAL);
         var blurFS:String = "sub ft0, v0, fc0\n" + "sub ft0.zw, ft0.zw, ft0.zw\n" + "dp3 ft1, ft0, ft0\n" + "sqt ft1, ft1\n" + "div ft1.xy, ft1.xy, fc0.zz\n" + "pow ft1.x, ft1.x, fc0.w\n" + "mul ft0.xy, ft0.xy, ft1.xx\n" + "div ft0.xy, ft0.xy, ft1.yy\n" + "add ft0.xy, ft0.xy, fc0.xy\n" + "tex oc, ft0, fs0<2d,clamp>\n";
         var blurVS:String = "m44 op, va0, vc0\n" + "mov v0, va1\n";
         assembler.assemble(Context3DProgramType.VERTEX,blurVS);
         var blurVSAGAL:ByteArray = assembler.agalcode;
         assembler.assemble(Context3DProgramType.FRAGMENT,blurFS);
         var blurFSAGAL:ByteArray = assembler.agalcode;
         this.blurPostProcessing_ = context3D.createProgram();
         this.blurPostProcessing_.upload(blurVSAGAL,blurFSAGAL);
         var shadowVS:String = "m44 op, va0, vc0\n" + "mov v0, va1\n" + "mov v1, va2\n";
         assembler.assemble(Context3DProgramType.VERTEX,shadowVS);
         var shadowVSAGAL:ByteArray = assembler.agalcode;
         // a radial gradient like Flash's (GameObject.drawShadow): the shadow's colour, its alpha
         // falling off evenly from the centre to nothing at the edge of the ellipse
         var shadowFS:String = "sub ft0.xy, v1.xy, fc4.xx\n" + "mul ft0.xy, ft0.xy, ft0.xy\n" + "add ft0.x, ft0.x, ft0.y\n"
            + "sqt ft0.x, ft0.x\n" + "mul ft0.x, ft0.x, fc4.y\n" + "sub ft0.x, fc4.z, ft0.x\n" + "sat ft0.x, ft0.x\n"
            + "mov ft1, v0\n" + "mul ft1.w, v0.w, ft0.x\n" + "mov oc, ft1\n";
         assembler.assemble(Context3DProgramType.FRAGMENT,shadowFS);
         var shadowFSAGAL:ByteArray = assembler.agalcode;
         this.shadowProgram_ = context3D.createProgram();
         this.shadowProgram_.upload(shadowVSAGAL,shadowFSAGAL);
         this.sceneTexture_ = context3D.createTexture(1024,1024,Context3DTextureFormat.BGRA,true);
         this.postFilterVertexBuffer_ = context3D.createVertexBuffer(4,4);
         this.postFilterVertexBuffer_.uploadFromVector(POST_FILTER_POSITIONS,0,4);
         this.postFilterIndexBuffer_ = context3D.createIndexBuffer(6);
         this.postFilterIndexBuffer_.uploadFromVector(POST_FILTER_TRIS,0,6);
         this.graphic3D_ = this.injector.getInstance(Graphic3D);
      }

      private function UpdateCameraMatrix(camera:Camera) : void
      {
         var angleRad:Number = -camera.angleRad_;
         this.f_.x = 0;
         this.f_.y = 0;
         this.f_.z = -1;
         this.p_.x = -(camera.x_ + this.widthOffset_);
         this.p_.y = camera.y_ - this.heightOffset_;
         this.p_.z = -camera.z_;
         this.r_.x = Math.cos(angleRad);
         this.r_.y = Math.sin(angleRad);
         this.r_.z = 0;
         this.u_.x = Math.cos(angleRad + Math.PI / 2);
         this.u_.y = Math.sin(angleRad + Math.PI / 2);
         this.u_.z = 0;
         this.rd_[0] = this.r_.x;
         this.rd_[1] = this.u_.x;
         this.rd_[2] = this.f_.x;
         this.rd_[3] = 0;
         this.rd_[4] = this.r_.y;
         this.rd_[5] = this.u_.y;
         this.rd_[6] = this.f_.y;
         this.rd_[7] = 0;
         this.rd_[8] = this.r_.z;
         this.rd_[9] = 1;
         this.rd_[10] = -this.f_.z;
         this.rd_[11] = 0;
         this.rd_[12] = this.p_.dotProduct(this.r_);
         this.rd_[13] = this.p_.dotProduct(this.u_);
         this.rd_[14] = -this.p_.dotProduct(this.f_);
         this.rd_[15] = 1;
         var temp:Matrix3D = new Matrix3D();
         temp.rawData = this.rd_;
         this.cameraMatrix_.identity();
         this.cameraMatrix_.append(temp);
      }

      private function onRender(graphicsDatas:Vector.<IGraphicsData>, grahpicsData3d:Vector.<Object3DStage3D>, mapWidth:Number, mapHeight:Number, camera:Camera, filterIndex:uint) : void
      {
         if(WebMain.STAGE.stageWidth != this.stageWidth || WebMain.STAGE.stageHeight != this.stageHeight)
         {
            this.resizeStage3DBackBuffer();
         }
         if(Renderer.inGame)
         {
            this.setTranslationToGame();
         }
         else
         {
            this.setTranslationToTitle();
         }
         var renderStart:int = getTimer();
         if(filterIndex > 0)
         {
            this.renderWithPostEffect(graphicsDatas,grahpicsData3d,mapWidth,mapHeight,camera,filterIndex);
         }
         else
         {
            this.renderScene(graphicsDatas,grahpicsData3d,mapWidth,mapHeight,camera);
         }
         this.context3D.present();
         renderMs += getTimer() - renderStart;
         WebMain.STAGE.scaleMode = StageScaleMode.NO_SCALE;
      }

      private function resizeStage3DBackBuffer() : void
      {
         if(WebMain.STAGE.stageWidth < 1 || WebMain.STAGE.stageHeight < 1)
         {
            return;
         }
         var stage3d:Stage3D = WebMain.STAGE.stage3Ds[0];
         stage3d.context3D.configureBackBuffer(WebMain.STAGE.stageWidth,WebMain.STAGE.stageHeight,2,true);
         this.stageWidth = WebMain.sWidth;
         this.stageHeight = WebMain.sHeight;
      }

      private function renderWithPostEffect(graphicsDatas:Vector.<IGraphicsData>, grahpicsData3d:Vector.<Object3DStage3D>, mapWidth:Number, mapHeight:Number, camera:Camera, filterIndex:uint) : void
      {
         this.context3D.GetContext3D().setRenderToTexture(this.sceneTexture_,true);
         this.renderScene(graphicsDatas,grahpicsData3d,mapWidth,mapHeight,camera);
         this.context3D.GetContext3D().setRenderToBackBuffer();
         switch(filterIndex)
         {
            case STAGE3D_FILTER_PAUSE:
            case STAGE3D_FILTER_BLIND:
               this.context3D.GetContext3D().setProgram(this.postProcessingProgram_);
               this.context3D.GetContext3D().setTextureAt(0,this.sceneTexture_);
               this.context3D.GetContext3D().clear(0.5,0.5,0.5);
               this.context3D.GetContext3D().setVertexBufferAt(0,this.postFilterVertexBuffer_,0,Context3DVertexBufferFormat.FLOAT_2);
               this.context3D.GetContext3D().setVertexBufferAt(1,null);
               break;
            case STAGE3D_FILTER_DRUNK:
               this.context3D.GetContext3D().setProgram(this.blurPostProcessing_);
               this.context3D.GetContext3D().setTextureAt(0,this.sceneTexture_);
               this.context3D.GetContext3D().clear(0.5,0.5,0.5);
               this.context3D.GetContext3D().setVertexBufferAt(0,this.postFilterVertexBuffer_,0,Context3DVertexBufferFormat.FLOAT_2);
               this.context3D.GetContext3D().setVertexBufferAt(1,this.postFilterVertexBuffer_,2,Context3DVertexBufferFormat.FLOAT_2);
         }
         this.context3D.GetContext3D().setVertexBufferAt(2,null);
         switch(filterIndex)
         {
            case STAGE3D_FILTER_PAUSE:
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.VERTEX,0,POST_FILTER_VERTEX_CONSTANTS);
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.FRAGMENT,0,GRAYSCALE_FRAGMENT_CONSTANTS);
               break;
            case STAGE3D_FILTER_BLIND:
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.VERTEX,0,POST_FILTER_VERTEX_CONSTANTS);
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.FRAGMENT,0,BLIND_FRAGMENT_CONSTANTS);
               break;
            case STAGE3D_FILTER_DRUNK:
               if(this.blurFragmentConstants_[3] <= 0.2 || this.blurFragmentConstants_[3] >= 1.8)
               {
                  this.blurFactor = this.blurFactor * -1;
               }
               this.blurFragmentConstants_[3] = this.blurFragmentConstants_[3] + this.blurFactor;
               this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,new Matrix3D());
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.FRAGMENT,0,this.blurFragmentConstants_,this.blurFragmentConstants_.length / 4);
         }
         this.context3D.GetContext3D().clear(0,0,0,1);
         this.context3D.GetContext3D().drawTriangles(this.postFilterIndexBuffer_);
      }

      private function renderScene(graphicsDatas:Vector.<IGraphicsData>, grahpicsData3d:Vector.<Object3DStage3D>, mapWidth:Number, mapHeight:Number, camera:Camera) : void
      {
         var test:int = 0;
         var graphicsData:IGraphicsData = null;
         this.context3D.clear();
         var finalTransform:Matrix3D = this.finalTransform_;
         var index3d:uint = 0;
         var ctx:Context3D = this.context3D.GetContext3D();
         this.widthOffset_ = -mapWidth / 2;
         this.heightOffset_ = mapHeight / 2;
         this.UpdateCameraMatrix(camera);
         drawCalls = drawsQuads = drawsCustom = drawsShadow = draws3D = 0;
         var count:int = graphicsDatas.length;
         var quads:int = batching ? this.prepareQuads(graphicsDatas) : 0;
         var quadOf:Vector.<int> = this.quadOf_;
         // what the batched quads need bound; anything drawn the original way unbinds it
         var bound:Boolean = false;
         var boundRepeat:Boolean = false;
         var boundImage:BitmapData = null;
         var boundPage:int = -1;
         var page:int = -1;
         var q:int = 0;
         var k:int = 0;
         var j:int = 0;
         var fill:GraphicsBitmapFill = null;
         var next:GraphicsBitmapFill = null;
         ctx.setCulling(Context3DTriangleFace.NONE);
         for(var gi:int = 0; gi < count; gi++)
         {
            q = quads > 0 ? quadOf[gi] : -1;
            if(q >= 0)
            {
               // a run of batched quads with the same image: one draw call
               fill = GraphicsBitmapFill(graphicsDatas[gi]);
               page = this.quadPage_[gi];
               k = 1;
               j = gi + 3;
               while(j < count && quadOf[j] == q + k)
               {
                  if(page >= 0)
                  {
                     // atlas quads: any image on the same page
                     if(this.quadPage_[j] != page)
                     {
                        break;
                     }
                  }
                  else
                  {
                     next = GraphicsBitmapFill(graphicsDatas[j]);
                     if(this.quadPage_[j] >= 0 || next.bitmapData != fill.bitmapData || next.repeat != fill.repeat)
                     {
                        break;
                     }
                  }
                  k++;
                  j = j + 3;
               }
               if(!bound || boundRepeat != fill.repeat)
               {
                  this.context3D.setProgram(Program3DFactory.getInstance().getProgram(this.context3D,fill.repeat));
                  boundRepeat = fill.repeat;
               }
               if(!bound)
               {
                  finalTransform.identity();
                  finalTransform.appendScale(1 / Stage3DConfig.HALF_WIDTH,1 / Stage3DConfig.HALF_HEIGHT,1);
                  finalTransform.appendTranslation(this.tX / Stage3DConfig.WIDTH,this.tY / Stage3DConfig.HEIGHT,0);
                  this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,finalTransform,true);
                  ctx.setProgramConstantsFromVector(Context3DProgramType.VERTEX,4,ZERO_OFFSET);
                  ctx.setVertexBufferAt(0,this.batchVB_,0,Context3DVertexBufferFormat.FLOAT_3);
                  ctx.setVertexBufferAt(1,this.batchVB_,3,Context3DVertexBufferFormat.FLOAT_2);
                  ctx.setVertexBufferAt(2,null);
                  bound = true;
                  boundImage = null;
                  boundPage = -1;
               }
               if(page >= 0)
               {
                  if(boundPage != page)
                  {
                     ctx.setTextureAt(0,this.atlasTextures_[page]);
                     this.graphic3D_.setIdentityColor(this.context3D);
                     boundPage = page;
                     boundImage = null;
                  }
               }
               else if(boundImage != fill.bitmapData)
               {
                  this.context3D.setTextureAt(0,this.graphic3D_.setTextureAndColor(fill.bitmapData,this.context3D));
                  boundImage = fill.bitmapData;
                  boundPage = -1;
               }
               ctx.drawTriangles(this.batchIB_,q * 6,k * 2);
               drawCalls++;
               drawsQuads++;
               gi = gi + k * 3 - 1;
               continue;
            }
            q = quads > 0 ? this.shadowOf_[gi] : -1;
            if(q >= 0)
            {
               // consecutive shadows: one draw call
               k = 1;
               j = gi + 3;
               while(j < count && this.shadowOf_[j] == q + k)
               {
                  k++;
                  j = j + 3;
               }
               ctx.setProgram(this.shadowProgram_);
               finalTransform.identity();
               finalTransform.appendTranslation(this.tX / Stage3DConfig.WIDTH,this.tY / Stage3DConfig.HEIGHT,0);
               this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,finalTransform,true);
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.FRAGMENT,4,SHADOW_CONSTANTS);
               ctx.setVertexBufferAt(0,this.shadowVB_,0,Context3DVertexBufferFormat.FLOAT_3);
               ctx.setVertexBufferAt(1,this.shadowVB_,3,Context3DVertexBufferFormat.FLOAT_4);
               ctx.setVertexBufferAt(2,this.shadowVB_,7,Context3DVertexBufferFormat.FLOAT_2);
               ctx.setTextureAt(0,null);
               ctx.drawTriangles(this.shadowIB_,q * 6,k * 2);
               drawCalls++;
               drawsShadow++;
               bound = false;
               gi = gi + k * 3 - 1;
               continue;
            }
            graphicsData = graphicsDatas[gi];
            if(graphicsData is GraphicsBitmapFill && !GraphicsFillExtra.isSoftwareDraw(GraphicsBitmapFill(graphicsData)))
            {
               try
               {
                  test = GraphicsBitmapFill(graphicsData).bitmapData.width;
               }
               catch(e:Error)
               {
                  //trace("ERROR CAUGHT -- Invalid Bitmap Data");
                  continue;
               }
               drawCalls++;
               drawsCustom++;
               bound = false;
               this.graphic3D_.setGraphic(GraphicsBitmapFill(graphicsData),this.context3D);
               finalTransform.identity();
               finalTransform.append(this.graphic3D_.getMatrix3D());
               finalTransform.appendScale(1 / Stage3DConfig.HALF_WIDTH,1 / Stage3DConfig.HALF_HEIGHT,1);
               finalTransform.appendTranslation(this.tX / Stage3DConfig.WIDTH,this.tY / Stage3DConfig.HEIGHT,0);
               this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,finalTransform,true);
               this.graphic3D_.render(this.context3D);
            }
            if(graphicsData is GraphicsGradientFill)
            {
               drawCalls++;
               drawsShadow++;
               bound = false;
               ctx.setProgram(this.shadowProgram_);
               this.graphic3D_.setGradientFill(GraphicsGradientFill(graphicsData),this.context3D,Stage3DConfig.HALF_WIDTH,Stage3DConfig.HALF_HEIGHT);
               finalTransform.identity();
               finalTransform.append(this.graphic3D_.getMatrix3D());
               finalTransform.appendTranslation(this.tX / Stage3DConfig.WIDTH,this.tY / Stage3DConfig.HEIGHT,0);
               this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,finalTransform,true);
               this.context3D.setProgramConstantsFromVector(Context3DProgramType.FRAGMENT,4,SHADOW_CONSTANTS);
               this.graphic3D_.renderShadow(this.context3D);
            }
            if(graphicsData == null && grahpicsData3d.length != 0)
            {
               try
               {
                  drawCalls++;
                  draws3D++;
                  bound = false;
                  ctx.setProgram(this.program2);
                  ctx.setCulling(Context3DTriangleFace.BACK);
                  grahpicsData3d[index3d].UpdateModelMatrix(this.widthOffset_,this.heightOffset_);
                  finalTransform.identity();
                  finalTransform.append(grahpicsData3d[index3d].GetModelMatrix());
                  finalTransform.append(this.cameraMatrix_);
                  finalTransform.append(this._projection);
                  finalTransform.appendTranslation(this.tX / Stage3DConfig.WIDTH,this.tY / Stage3DConfig.HEIGHT * 11.5,0);
                  this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,0,finalTransform,true);
                  this.context3D.setProgramConstantsFromMatrix(Context3DProgramType.VERTEX,8,grahpicsData3d[index3d].GetModelMatrix(),true);
                  grahpicsData3d[index3d].draw(ctx);
                  index3d++;
               }
               catch(e:Error)
               {
                  trace("ERROR CAUGHT -- Invalid Bitmap Data");
               }
               ctx.setCulling(Context3DTriangleFace.NONE);
            }
         }
      }

      // Puts every plain quad of the frame (a bitmap fill drawn as Graphic3D's unit quad, without
      // a vertex buffer of its own) into one vertex buffer, uploaded once: its four corners placed
      // as Graphic3D.transform places them, with the fill's texture offset or sink baked into the
      // texture coordinates (Graphic3D passes those as a constant instead). quadOf_ gets each
      // entry's quad number, or -1. Returns the number of quads.
      private function prepareQuads(datas:Vector.<IGraphicsData>) : int
      {
         var count:int = datas.length;
         var quadOf:Vector.<int> = this.quadOf_;
         quadOf.length = count;
         var quadPage:Vector.<int> = this.quadPage_;
         quadPage.length = count;
         var shadowOf:Vector.<int> = this.shadowOf_;
         shadowOf.length = count;
         var sd:Vector.<Number> = this.shadowData_;
         var shadows:int = 0;
         var sp:int = 0;
         var gradient:GraphicsGradientFill = null;
         var shadowColor:uint = 0;
         var shadowR:Number = 0;
         var shadowG:Number = 0;
         var shadowB:Number = 0;
         var shadowA:Number = 0;
         var gm:Matrix = null;
         var k:int = 0;
         var ctx:Context3D = this.context3D.GetContext3D();
         this.atlas_.beginFrame(ctx);
         var cell:int = 0;
         var cx0:Number = 0;
         var cy0:Number = 0;
         var su:Number = 1;
         var sv:Number = 1;
         var ct:ColorTransform = null;
         var d:Vector.<Number> = this.batchData_;
         var n:int = 0;
         var p:int = 0;
         var fill:GraphicsBitmapFill = null;
         var bmp:BitmapData = null;
         var m:Matrix = null;
         var w:Number = 0;
         var h:Number = 0;
         var ax:Number = 0;
         var ay:Number = 0;
         var cx:Number = 0;
         var cy:Number = 0;
         var u:Number = 0;
         var v:Number = 0;
         var sink:Number = 0;
         var offset:Vector.<Number> = null;
         for(var i:int = 0; i < count; i++)
         {
            quadOf[i] = -1;
            quadPage[i] = -1;
            shadowOf[i] = -1;
            gradient = datas[i] as GraphicsGradientFill;
            if(gradient != null)
            {
               if(shadows < BATCH_SHADOWS)
               {
                  gm = gradient.matrix;
                  shadowColor = uint(gradient.colors[0]);
                  shadowR = (shadowColor >> 16 & 0xFF) / 255;
                  shadowG = (shadowColor >> 8 & 0xFF) / 255;
                  shadowB = (shadowColor & 0xFF) / 255;
                  shadowA = Number(gradient.alphas[0]);
                  if(sd.length < sp + 36)
                  {
                     sd.length = sp + 36 * 64;
                  }
                  for(k = 0; k < 4; k++)
                  {
                     sd[sp++] = gm.a * 1.5 * SHADOW_X[k] - gm.c * SHADOW_Y[k] + gm.tx / Stage3DConfig.HALF_WIDTH;
                     sd[sp++] = -gm.b * SHADOW_X[k] + gm.d * 1.5 * SHADOW_Y[k] - gm.ty / Stage3DConfig.HALF_HEIGHT;
                     sd[sp++] = 0;
                     sd[sp++] = shadowR;
                     sd[sp++] = shadowG;
                     sd[sp++] = shadowB;
                     sd[sp++] = shadowA;
                     sd[sp++] = SHADOW_U[k];
                     sd[sp++] = SHADOW_V[k];
                  }
                  shadowOf[i] = shadows;
                  shadows++;
               }
               continue;
            }
            fill = datas[i] as GraphicsBitmapFill;
            if(fill == null || n >= BATCH_QUADS || GraphicsFillExtra.isSoftwareDraw(fill) || GraphicsFillExtra.getVertexBuffer(fill) != null)
            {
               continue;
            }
            bmp = fill.bitmapData;
            if(bmp == null)
            {
               continue;
            }
            try
            {
               // the texture's size: the image's, rounded up to powers of two (TextureFactory)
               w = nextPowerOf2(bmp.width);
               h = nextPowerOf2(bmp.height);
            }
            catch(e:Error)
            {
               continue; // disposed image
            }
            sink = GraphicsFillExtra.getSinkLevel(fill);
            if(sink != 0)
            {
               u = 0;
               v = -sink;
            }
            else
            {
               offset = GraphicsFillExtra.getOffsetUV(fill);
               u = offset[0];
               v = offset[1];
            }
            // in the atlas: texture coordinates within its cell instead of the whole texture
            cx0 = 0;
            cy0 = 0;
            su = 1;
            sv = 1;
            if(atlasing && !fill.repeat && u == 0 && v == 0)
            {
               // only a look: a tinted image is drawn on its own, which applies (and uses up) it
               ct = GraphicsFillExtra.peekColorTransform(bmp);
               if(ct.redMultiplier == 1 && ct.greenMultiplier == 1 && ct.blueMultiplier == 1 && ct.alphaMultiplier == 1 && ct.redOffset == 0 && ct.greenOffset == 0 && ct.blueOffset == 0 && ct.alphaOffset == 0)
               {
                  cell = this.atlas_.cellOf(bmp,w,h);
                  if(cell >= 0)
                  {
                     quadPage[i] = int(cell / 16777216);
                     cx0 = int(cell / 4096) % 4096 / TextureAtlas.PAGE;
                     cy0 = cell % 4096 / TextureAtlas.PAGE;
                     su = w / TextureAtlas.PAGE;
                     sv = h / TextureAtlas.PAGE;
                  }
               }
            }
            m = fill.matrix;
            ax = m.a * w;
            ay = m.b * w;
            cx = m.c * h;
            cy = m.d * h;
            if(d.length < p + 20)
            {
               d.length = p + 20 * 64;
            }
            // texture pixels (0,0), (w,0), (0,h) and (w,h) through the fill's matrix, y flipped
            d[p++] = m.tx; d[p++] = -m.ty; d[p++] = 0; d[p++] = cx0 + u * su; d[p++] = cy0 + v * sv;
            d[p++] = m.tx + ax; d[p++] = -(m.ty + ay); d[p++] = 0; d[p++] = cx0 + (1 + u) * su; d[p++] = cy0 + v * sv;
            d[p++] = m.tx + cx; d[p++] = -(m.ty + cy); d[p++] = 0; d[p++] = cx0 + u * su; d[p++] = cy0 + (1 + v) * sv;
            d[p++] = m.tx + ax + cx; d[p++] = -(m.ty + ay + cy); d[p++] = 0; d[p++] = cx0 + (1 + u) * su; d[p++] = cy0 + (1 + v) * sv;
            quadOf[i] = n;
            n++;
         }
         if(n == 0 && shadows == 0)
         {
            return 0;
         }
         this.atlasTextures_ = this.atlas_.upload();
         if(this.batchCtx_ != ctx || this.batchVB_ == null)
         {
            // (again after the GPU context is lost and recreated)
            this.batchCtx_ = ctx;
            this.batchVB_ = ctx.createVertexBuffer(BATCH_QUADS * 4,5);
            this.batchIB_ = ctx.createIndexBuffer(BATCH_QUADS * 6);
            var indices:Vector.<uint> = new Vector.<uint>(BATCH_QUADS * 6,true);
            for(var q:int = 0; q < BATCH_QUADS; q++)
            {
               indices[q * 6] = q * 4;
               indices[q * 6 + 1] = q * 4 + 1;
               indices[q * 6 + 2] = q * 4 + 2;
               indices[q * 6 + 3] = q * 4 + 2;
               indices[q * 6 + 4] = q * 4 + 1;
               indices[q * 6 + 5] = q * 4 + 3;
            }
            this.batchIB_.uploadFromVector(indices,0,BATCH_QUADS * 6);
            // Stage3D treats a vertex buffer as invalid (Error #3608, and silently skips the draw
            // without error checking) until all of it has been uploaded once
            this.batchVB_.uploadFromVector(new Vector.<Number>(BATCH_QUADS * 4 * 5,true),0,BATCH_QUADS * 4);
            this.shadowVB_ = ctx.createVertexBuffer(BATCH_SHADOWS * 4,9);
            this.shadowVB_.uploadFromVector(new Vector.<Number>(BATCH_SHADOWS * 4 * 9,true),0,BATCH_SHADOWS * 4);
            // its own index buffer: Stage3D checks every index against the vertex buffer's size
            this.shadowIB_ = ctx.createIndexBuffer(BATCH_SHADOWS * 6);
            this.shadowIB_.uploadFromVector(indices,0,BATCH_SHADOWS * 6);
         }
         if(n > 0)
         {
            this.batchVB_.uploadFromVector(d,0,n * 4);
         }
         if(shadows > 0)
         {
            this.shadowVB_.uploadFromVector(sd,0,shadows * 4);
         }
         return n + shadows;
      }

      private static function nextPowerOf2(value:int) : int
      {
         value--;
         value = value | value >> 1;
         value = value | value >> 2;
         value = value | value >> 4;
         value = value | value >> 8;
         value = value | value >> 16;
         return value + 1;
      }

      private function setTranslationToGame() : void
      {
         this.tX = 0;
         this.tX = -200 / Parameters.data_.mscale * (WebMain.STAGE.stageHeight / 600);
         this.tY = !Parameters.data_.centerOnPlayer?Number(Number((Camera.OFFSET_SCREEN_RECT.y + Camera.CENTER_SCREEN_RECT.height / 2) * 2)):Number(Number(-50 / Parameters.data_.mscale * (WebMain.STAGE.stageHeight / 600)));
      }

      private function setTranslationToTitle() : void
      {
         this.tX = -200;
         this.tY = 0;
      }
   }
}
